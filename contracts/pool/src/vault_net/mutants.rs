// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
//! Filet de tests du vault : tests qui tuent les mutants survivants (angle « mutation »)
//! et tests rouges / garde-fous des lots B5 (plafond senior) et B6 (plancher de parts junior).
use super::*;
use crate::{Pool, PoolClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{symbol_short, token};

const USDC: i128 = 10_000_000;
const YEAR: u64 = 31_536_000;

struct Net {
    env: Env,
    pool: Address,
    usdc: Address,
}

fn net() -> Net {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let oracle = env.register(mock_oracle::MockOracle, (&admin,));
    let pool = env.register(Pool, (&admin, &usdc, &oracle, &50u32));
    Net { env, pool, usdc }
}

impl Net {
    fn c(&self) -> PoolClient<'_> {
        PoolClient::new(&self.env, &self.pool)
    }
    fn vault<R>(&self, f: impl FnOnce(&Env) -> R) -> R {
        self.env.as_contract(&self.pool, || f(&self.env))
    }
    fn t(&self) -> i128 {
        self.vault(total_deposits)
    }
    fn r(&self) -> i128 {
        self.vault(reserved)
    }
    fn bal(&self, who: &Address) -> i128 {
        token::Client::new(&self.env, &self.usdc).balance(who)
    }
    fn mint(&self, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&self.env, &self.usdc).mint(to, &amount);
    }
    /// LP autorisé (liste blanche active par défaut), avec `amount` USDC en poche.
    fn lp(&self, amount: i128) -> Address {
        let lp = Address::generate(&self.env);
        self.c().set_allowed(&Role::Lp, &lp, &true);
        self.mint(&lp, amount);
        lp
    }
    fn junior(&self, amount: i128) -> Address {
        let lp = self.lp(amount);
        self.c().deposit_lp(&lp, &amount);
        lp
    }
    fn senior(&self, amount: i128) -> Address {
        let lp = self.lp(amount);
        self.c().deposit_senior(&lp, &amount);
        lp
    }
    /// La PME envoie la marge au contrat, le vault la réserve (comme hedge::open).
    fn open_margin(&self, margin: i128) -> Address {
        let sme = Address::generate(&self.env);
        self.mint(&sme, margin);
        token::Client::new(&self.env, &self.usdc).transfer(&sme, &self.pool, &margin);
        self.vault(|env| reserve(env, margin));
        sme
    }
    /// Réserve toute la capacité (marge = capacity(), avec ou sans provision B5).
    fn saturate(&self) -> (Address, i128) {
        let m = self.c().capacity();
        (self.open_margin(m), m)
    }
    /// Règlement à P&L nul : réserve libérée, marge rendue.
    fn settle_flat(&self, sme: &Address, margin: i128) {
        self.vault(|env| release(env, margin));
        self.pay(sme, margin);
    }
    /// Règlement perdant pour le pool : `loss` payé à la PME en plus de sa marge.
    fn settle_loss(&self, sme: &Address, margin: i128, loss: i128) {
        self.vault(|env| {
            release(env, margin);
            book_loss(env, loss);
        });
        self.pay(sme, margin + loss);
    }
    fn pay(&self, to: &Address, amount: i128) {
        self.env.as_contract(&self.pool, || {
            token::Client::new(&self.env, &self.usdc).transfer(&self.pool, to, &amount)
        });
    }
    /// Valeur d'un LP junior au prix plein, calculée sans passer par le prix arrondi à 7 décimales.
    fn junior_worth(&self, lp: &Address) -> i128 {
        let shares = self.c().lp_shares(lp);
        self.vault(|env| mul_div(env, shares, junior_value(env), total_shares(env)))
    }
    fn advance(&self, secs: u64) {
        self.env.ledger().with_mut(|l| l.timestamp += secs);
    }
    /// Invariant de solde + règle des tranches, au repos (`owed` = Σ marges ouvertes).
    fn assert_state(&self, owed: i128) {
        assert_eq!(self.bal(&self.pool), self.t() + owed, "solde USDC = T + dû aux PME");
        let tv = self.c().tranche_values();
        assert_eq!(tv.senior + tv.junior, self.t(), "senior + junior = T");
        assert!(self.c().exit_price() <= self.c().share_price(), "prix de sortie ≤ prix plein");
    }
}

// =====================================================================================
// 1. Tests qui tuent les mutants survivants du code actuel (tous VERTS)
// =====================================================================================

/// Tue M03 (montant de `withdraw` arrondi vers le haut). Capital libre non divisible :
/// 3 USDC − 1 stroop pour 3 USDC de parts ; A (1/3) doit recevoir le plancher, 9 999 999.
#[test]
fn withdraw_rounds_down_and_equals_balance() {
    let s = net();
    let a = s.junior(1 * USDC);
    s.junior(2 * USDC);
    s.open_margin(1);
    let before = s.c().lp_balance(&a);
    assert_eq!(before, 9_999_999);
    assert_eq!(s.c().withdraw(&a, &(1 * USDC)), before);
    s.assert_state(1);
}

/// Tue M04 (montant de `withdraw_senior` arrondi vers le haut). Prix senior non rond
/// (1 001 s d'intérêts), deux seniors de 1 USDC et 1 USDC + 1 stroop : le retrait = `senior_balance`.
#[test]
fn withdraw_senior_rounds_down_and_equals_balance() {
    let s = net();
    s.junior(1_000 * USDC);
    let a = s.senior(1 * USDC);
    s.senior(1 * USDC + 1);
    s.advance(1_001);
    let shares = s.vault(|env| senior_shares(env, &a));
    let total = s.vault(senior_total_shares);
    let value = s.vault(senior_value);
    // Contrôle du scénario : il y a bien une fraction à arrondir.
    assert_ne!((shares * value) % total, 0, "scénario à revoir : division exacte");
    let before = s.c().senior_balance(&a);
    assert_eq!(s.c().withdraw_senior(&a, &shares), before);
    s.assert_state(0);
}

/// Tue M12 (`withdraw_senior` ne capitalise plus le prix). A sort au bout d'un an ; un an plus
/// tard, le prix doit être 1,045 × 1,045 = 1,092025 (capitalisé au retrait de A), pas 1,09.
#[test]
fn withdraw_senior_capitalises_price_for_remaining_seniors() {
    let s = net();
    s.junior(1_000_000 * USDC);
    let a = s.senior(400_000 * USDC);
    let b = s.senior(400_000 * USDC);
    s.advance(YEAR);
    s.c().withdraw_senior(&a, &(400_000 * USDC));
    s.advance(YEAR);
    assert_eq!(s.c().tranche_values().senior_price, 10_920_250);
    let v = s.c().senior_balance(&b);
    assert!(v <= 436_810 * USDC && 436_810 * USDC - v <= 2, "B ≈ 400 000 × 1,092025 : {v}");
    s.assert_state(0);
}

/// Tue M15 (`set_exposure` ne supprime plus la clé) : après la dernière clôture, plus d'entrée.
#[test]
fn closing_last_position_removes_exposure_key() {
    let s = net();
    s.junior(100_000 * USDC);
    let h = Hedge {
        owner: Address::generate(&s.env),
        supplier: Address::generate(&s.env),
        currency: symbol_short!("MXN"),
        side: Side::BuyCurrency,
        notional: 10_000_000_000_000,
        locked_rate: 5_500_000_000_000,
        cap_bps: 1000,
        margin: 5_500 * USDC,
        maturity: 0,
        settled: false,
    };
    s.mint(&h.owner, h.margin);
    token::Client::new(&s.env, &s.usdc).transfer(&h.owner, &s.pool, &h.margin);
    s.vault(|env| open_position(env, &h));
    let key = VaultKey::Exposure(symbol_short!("MXN"));
    assert!(s.vault(|env| env.storage().persistent().has(&key)));
    s.vault(|env| close_position(env, &h));
    assert!(!s.vault(|env| env.storage().persistent().has(&key)), "clé d'exposition supprimée");
}

/// Tue M28 (`set_lp_shares` garde une clé à 0) : sortie totale = entrée supprimée.
#[test]
fn full_exit_removes_shares_key() {
    let s = net();
    let lp = s.junior(1_000 * USDC);
    s.c().withdraw(&lp, &(1_000 * USDC));
    let key = VaultKey::Shares(lp.clone());
    assert!(!s.vault(|env| env.storage().persistent().has(&key)));
}

/// Tue M18 (dépôt junior évalué sur T, senior compris) : avec 800 000 de senior, 100 000
/// déposés au prix junior de 1 donnent 100 000 parts.
#[test]
fn junior_deposit_after_senior_is_priced_on_junior_value() {
    let s = net();
    s.junior(200_000 * USDC);
    s.senior(800_000 * USDC);
    let lp = s.lp(100_000 * USDC);
    assert_eq!(s.c().deposit_lp(&lp, &(100_000 * USDC)), 100_000 * USDC);
    assert_eq!(s.junior_worth(&lp), 100_000 * USDC);
    s.assert_state(0);
}

/// Tue M19 (ratio du retrait junior calculé sur T avant retrait). 250 000 / 800 000 : retirer
/// 50 000 laisse 200 000 / 1 000 000, 20 % pile, accepté ; 1 stroop de plus est refusé (111).
/// Suppose prix de sortie = 1 sans couverture : à revoir si la provision B5 réduit le prix de sortie.
#[test]
fn junior_withdraw_down_to_exact_ratio_is_accepted() {
    let s = net();
    let j = s.junior(250_000 * USDC);
    s.senior(800_000 * USDC);
    assert_eq!(s.c().try_withdraw(&j, &(50_000 * USDC + 1)).unwrap_err().unwrap(), VaultError::JuniorRatioTooLow.into());
    assert_eq!(s.c().withdraw(&j, &(50_000 * USDC)), 50_000 * USDC);
    s.assert_state(0);
}

/// Tue M22 (garde de `close_position` en `&&`) : notionnel connu mais marge plus grande que
/// celle de l'exposition → 109, pas une marge d'exposition négative.
#[test]
#[should_panic(expected = "Error(Contract, #109)")]
fn close_position_with_unknown_margin_panics() {
    let s = net();
    s.junior(100_000 * USDC);
    let mut h = Hedge {
        owner: Address::generate(&s.env),
        supplier: Address::generate(&s.env),
        currency: symbol_short!("MXN"),
        side: Side::BuyCurrency,
        notional: 10_000_000_000_000,
        locked_rate: 5_500_000_000_000,
        cap_bps: 1000,
        margin: 5_500 * USDC,
        maturity: 0,
        settled: false,
    };
    s.mint(&h.owner, h.margin);
    token::Client::new(&s.env, &s.usdc).transfer(&h.owner, &s.pool, &h.margin);
    s.vault(|env| open_position(env, &h));
    h.margin += 1;
    s.vault(|env| close_position(env, &h));
}

/// Tue M30 (`senior_balance` sur la valeur promise, sans plafond) : junior épuisé + un an,
/// le senior ne peut lire que le capital restant.
#[test]
fn senior_balance_capped_when_junior_is_exhausted() {
    let s = net();
    s.junior(200_000 * USDC);
    let sr = s.senior(800_000 * USDC);
    let (sme, m) = s.saturate();
    s.settle_loss(&sme, m, m);
    s.advance(YEAR);
    // Créance 836 000 > T : la lecture senior est bornée au capital restant.
    assert_eq!(s.c().senior_balance(&sr), s.t());
}

/// Branches d'erreur jamais atteintes : 105 sur `deposit_senior`, 107 sur `withdraw_senior`.
#[test]
fn senior_error_branches() {
    let s = net();
    s.junior(1_000 * USDC);
    let a = s.senior(1 * USDC);
    s.advance(YEAR);
    // Prix 1,045 : 1 stroop donne 0 part.
    let tiny = s.lp(1);
    assert_eq!(s.c().try_deposit_senior(&tiny, &1).unwrap_err().unwrap(), VaultError::DepositTooSmall.into());
    // État forcé : tout le capital perdu (T = 0), la valeur senior tombe à 0.
    let all = s.t();
    s.vault(|env| book_loss(env, all));
    assert_eq!(s.c().try_withdraw_senior(&a, &1).unwrap_err().unwrap(), VaultError::WithdrawTooSmall.into());
}

// =====================================================================================
// 2. Défauts connus : ROUGES aujourd'hui, verts après B6 / B5
// =====================================================================================

/// Défaut 1 (B6) : inflation par la décote de sortie. Robuste au correctif : soit le retrait
/// qui laisse la poussière est refusé (116), soit la victime perd au plus 10 000 stroops (arrondi).
#[test]
#[ignore = "ROUGE jusqu'à B6 : plancher de parts junior"]
fn exit_discount_inflation_is_blocked() {
    let s = net();
    let attacker = s.junior(1_000 * USDC);
    let sme = s.open_margin(500 * USDC);
    let attack = s.c().try_withdraw(&attacker, &(1_000 * USDC - 1));
    let victim = s.lp(999 * USDC);
    s.c().deposit_lp(&victim, &(999 * USDC));
    s.settle_flat(&sme, 500 * USDC);
    let value = s.junior_worth(&victim);
    // Avec un plancher de 1 USDC de parts, l'arrondi coûte au plus quelques centaines de stroops.
    assert!(
        attack.is_err() || value >= 999 * USDC - 10_000,
        "la victime a perdu {} stroops",
        999 * USDC - value
    );
    s.assert_state(0);
}

/// Défaut 2 (B5) : intérêts senior qui font passer le junior sous R ; le retrait senior total
/// laisse T < R et le règlement perdant échoue en 103. Après B5 : retrait limité à 810 000.
#[test]
#[ignore = "ROUGE jusqu'à B5 : valeur senior payable plafonnée à T − R"]
fn senior_exit_under_cap_keeps_losing_settlement_payable() {
    let s = net();
    s.junior(200_000 * USDC);
    let sr = s.senior(800_000 * USDC);
    let (sme, m) = s.saturate(); // 190 000 sans provision
    s.advance(YEAR);
    let payable = s.t() - s.r();
    // La lecture affichée au front = ce que le retrait versera (tue « senior_balance sur la créance »).
    assert_eq!(s.c().senior_balance(&sr), payable);
    let paid = s.c().withdraw_senior(&sr, &(800_000 * USDC));
    assert!(s.t() >= s.r(), "T = {} < R = {}", s.t(), s.r());
    assert_eq!(paid, payable); // 810 000 sans provision
    s.settle_loss(&sme, m, m);
    s.assert_state(0);
}

/// B5 : capacité jamais négative (aujourd'hui −26 000 dans ce scénario).
#[test]
#[ignore = "ROUGE jusqu'à B5 : capacity() ≥ 0"]
fn capacity_never_negative_under_senior_cap() {
    let s = net();
    s.junior(200_000 * USDC);
    s.senior(800_000 * USDC);
    s.saturate();
    s.advance(YEAR);
    assert!(s.c().capacity() >= 0, "capacité = {}", s.c().capacity());
}

/// B6 : le plancher passe AVANT le calcul du montant (sinon 107 masque 116).
#[test]
#[ignore = "ROUGE jusqu'à B6 : code 116 avant 107"]
fn full_exit_with_all_capital_reserved_reports_116() {
    let s = net();
    let lp = s.junior(100 * USDC);
    s.open_margin(100 * USDC);
    let err = s.c().try_withdraw(&lp, &(100 * USDC)).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(116));
}

/// B6 : le plancher porte sur la poussière (1 stroop restant), pas seulement sur 0 part.
#[test]
#[ignore = "ROUGE jusqu'à B6 : poussière sous le plancher refusée"]
fn leaving_one_stroop_of_shares_with_reserve_reports_116() {
    let s = net();
    let lp = s.junior(1_000 * USDC);
    s.open_margin(500 * USDC);
    let err = s.c().try_withdraw(&lp, &(1_000 * USDC - 1)).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(116));
}

// =====================================================================================
// 3. Garde-fous VERTS aujourd'hui, qui tueront les mauvaises implémentations de B5 / B6
// =====================================================================================

/// B6, mutant « borne en <= » : laisser exactement 1 USDC de parts (le plancher) est accepté.
#[test]
fn leaving_exactly_the_floor_is_accepted() {
    let s = net();
    let lp = s.junior(1_000 * USDC);
    s.open_margin(500 * USDC);
    s.c().withdraw(&lp, &(999 * USDC));
    assert_eq!(s.c().lp_shares(&lp), 1 * USDC);
    s.assert_state(500 * USDC);
}

/// B6, mutant « plancher sur les parts du LP au lieu du total » : A sort entièrement, B garde
/// ses parts, couverture ouverte : accepté.
#[test]
fn one_lp_full_exit_while_another_stays_is_accepted() {
    let s = net();
    let a = s.junior(1_000 * USDC);
    s.junior(1_000 * USDC);
    s.open_margin(500 * USDC);
    assert_eq!(s.c().withdraw(&a, &(1_000 * USDC)), 750 * USDC);
    s.assert_state(500 * USDC);
}

/// B6, mutant « plancher sans condition R > 0 » : sortie totale après règlement acceptée.
#[test]
fn full_exit_after_settlement_is_accepted() {
    let s = net();
    let lp = s.junior(1_000 * USDC);
    let sme = s.open_margin(500 * USDC);
    s.settle_flat(&sme, 500 * USDC);
    assert_eq!(s.c().withdraw(&lp, &(1_000 * USDC)), 1_000 * USDC);
    assert_eq!(s.t(), 0);
    s.assert_state(0);
}

/// B5, défaut 3 / mutant « plafond appliqué au prix junior » : un dépôt junior pendant que le
/// plafond joue est évalué sur la créance non plafonnée (valeur ≥ dépôt − 2 stroops).
#[test]
fn junior_deposit_under_senior_cap_is_not_overcharged() {
    let s = net();
    s.junior(200_000 * USDC);
    s.senior(800_000 * USDC);
    let (_, m) = s.saturate();
    s.advance(YEAR);
    let lp = s.lp(100_000 * USDC);
    s.c().deposit_lp(&lp, &(100_000 * USDC));
    let value = s.junior_worth(&lp);
    assert!(value >= 100_000 * USDC - 2, "dépôt surpayé de {} stroops", 100_000 * USDC - value);
    // Créance senior intacte : 836 000 (non plafonnée) dans le prix et le ratio.
    assert_eq!(s.c().tranche_values().senior, 836_000 * USDC);
    s.assert_state(m);
}

/// B5, mutant « provision d'intérêts sans senior » : sans part senior, capacité = T − R pile.
#[test]
fn without_senior_capacity_is_exactly_t_minus_r() {
    let s = net();
    s.junior(1_000 * USDC);
    s.open_margin(300 * USDC);
    s.advance(YEAR);
    assert_eq!(s.c().capacity(), 700 * USDC);
    assert_eq!(s.c().exit_price(), 7_000_000);
}

/// B5, mutant « payable figé » : après un gain qui reconstitue le junior, la lecture senior
/// rejoint la créance (rattrapage). Vert aujourd'hui (pas de plafond).
#[test]
fn senior_balance_catches_up_after_pool_gain() {
    let s = net();
    s.junior(200_000 * USDC);
    let sr = s.senior(800_000 * USDC);
    let (_, m) = s.saturate();
    s.advance(YEAR);
    // Gain du pool : la PME perd toute sa marge.
    s.vault(|env| {
        release(env, m);
        book_gain(env, m);
    });
    assert_eq!(s.c().senior_balance(&sr), 836_000 * USDC);
    s.assert_state(0);
}

/// B5, provision d'intérêts (mutant « provision oubliée ») : capacité saturée 120 jours puis
/// règlement perdant à pleine marge ; le senior qui sort est servi en entier (créance).
#[test]
#[ignore = "ROUGE jusqu'à B5 : provision de 120 jours d'intérêts dans la capacité"]
fn senior_served_in_full_after_120_days_saturated() {
    let s = net();
    s.junior(200_000 * USDC);
    let sr = s.senior(800_000 * USDC);
    let (sme, m) = s.saturate();
    s.advance(120 * 86_400);
    s.settle_loss(&sme, m, m);
    let claim = s.vault(|env| mul_div(env, senior_total_shares(env), senior_price_now(env), SENIOR_PRICE_ONE));
    let paid = s.c().withdraw_senior(&sr, &(800_000 * USDC));
    assert!(paid >= claim - 2, "senior servi {paid} sur une créance de {claim}");
    s.assert_state(0);
}
