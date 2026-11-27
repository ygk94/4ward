// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
//! Filet de tests « attaquant » : scénarios déterministes chiffrés au stroop.
//! Chaque test raconte qui gagne, qui perd et combien.
//! Statuts : VERT (passe et doit continuer), CONSTAT (vert aujourd'hui, fige la perte exacte
//! du défaut : à supprimer ou inverser avec le correctif), ROUGE (`#[ignore]` jusqu'au correctif),
//! FUTUR (dépend d'un lot pas encore écrit).
use super::*;
use crate::{Pool, PoolClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token;

/// 1 USDC en 7 décimales.
const USDC: i128 = 10_000_000;
const YEAR: u64 = 31_536_000;
const DAY: u64 = 86_400;

// Codes des lots à venir : numéros du plan (docs/plan-vault-hugo.md), à aligner une fois figés.
/// B6 : retrait qui passerait sous le plancher de parts junior avec Reserved > 0.
const LAST_SHARE_LOCKED: u32 = 116;
/// B5 : dépôt senior pendant que la valeur payable est plafonnée.
const SENIOR_ARREARS: u32 = 118;
/// B3 : dépôt refusé tant qu'une couverture échue n'est pas réglée (113 ou 114 selon le plan).
const MATURED_HEDGE_PENDING: u32 = 113;

fn code(n: u32) -> soroban_sdk::Error {
    soroban_sdk::Error::from_contract_error(n)
}

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
    PoolClient::new(&env, &pool).set_senior_deposits_open(&true); // fermés par défaut (B5)
    Net { env, pool, usdc }
}

impl Net {
    fn c(&self) -> PoolClient<'_> {
        PoolClient::new(&self.env, &self.pool)
    }

    fn v<R>(&self, f: impl FnOnce(&Env) -> R) -> R {
        self.env.as_contract(&self.pool, || f(&self.env))
    }

    fn mint(&self, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&self.env, &self.usdc).mint(to, &amount);
    }

    fn usdc_of(&self, who: &Address) -> i128 {
        token::Client::new(&self.env, &self.usdc).balance(who)
    }

    /// LP autorisé (liste blanche active par défaut), sans fonds.
    fn lp(&self) -> Address {
        let lp = Address::generate(&self.env);
        self.c().set_allowed(&Role::Lp, &lp, &true);
        lp
    }

    /// Dépôt junior de `amount` par `lp` (fonds créés juste avant). Retourne les parts émises.
    fn deposit(&self, lp: &Address, amount: i128) -> i128 {
        self.mint(lp, amount);
        let before = self.c().lp_shares(lp);
        self.c().deposit_lp(lp, &amount) - before
    }

    fn junior(&self, amount: i128) -> Address {
        let lp = self.lp();
        self.deposit(&lp, amount);
        lp
    }

    fn senior(&self, amount: i128) -> Address {
        let lp = self.lp();
        self.mint(&lp, amount);
        self.c().deposit_senior(&lp, &amount);
        lp
    }

    /// Une PME poste `margin` et le vault la réserve (comme hedge::open).
    fn open(&self, margin: i128) -> Address {
        let sme = Address::generate(&self.env);
        self.mint(&sme, margin);
        token::Client::new(&self.env, &self.usdc).transfer(&sme, &self.pool, &margin);
        self.v(|env| reserve(env, margin));
        sme
    }

    fn pay(&self, to: &Address, amount: i128) {
        if amount > 0 {
            self.env.as_contract(&self.pool, || {
                token::Client::new(&self.env, &self.usdc).transfer(&self.pool, to, &amount)
            });
        }
    }

    /// Règlement à P&L nul : marge rendue.
    fn close_flat(&self, sme: &Address, margin: i128) {
        self.v(|env| release(env, margin));
        self.pay(sme, margin);
    }

    /// Règlement gagnant pour le pool (même ordre que hedge::close).
    fn close_pool_wins(&self, sme: &Address, margin: i128, gain: i128) {
        self.v(|env| release(env, margin));
        self.v(|env| book_gain(env, gain));
        self.pay(sme, margin - gain);
    }

    /// Règlement perdant pour le pool : `book_loss` échoue en 103 si T < perte.
    fn close_pool_loses(&self, sme: &Address, margin: i128, loss: i128) {
        self.v(|env| release(env, margin));
        self.v(|env| book_loss(env, loss));
        self.pay(sme, margin + loss);
    }

    fn t(&self) -> i128 {
        self.v(total_deposits)
    }

    fn r(&self) -> i128 {
        self.v(reserved)
    }

    fn advance(&self, secs: u64) {
        self.env.ledger().with_mut(|l| l.timestamp += secs);
    }

    /// Solde USDC du contrat = T + marges ouvertes + réclamables (chaque `open` poste exactement sa
    /// réserve ; B4 : `settle` crédite au lieu de transférer).
    fn check(&self) {
        assert_eq!(self.usdc_of(&self.pool), self.t() + self.r() + self.c().total_claimable(), "solde USDC = T + R + réclamables");
    }

    /// Retire toutes les parts junior de `lp`, retourne l'USDC versé.
    fn exit_junior(&self, lp: &Address) -> i128 {
        self.c().withdraw(lp, &self.c().lp_shares(lp))
    }
}

// ---------------------------------------------------------------------------------------------
// Défaut 1 (B6) : inflation par la décote de sortie
// ---------------------------------------------------------------------------------------------

/// Résultat d'un scénario d'inflation : (perte de la victime, gain de l'attaquant), en stroops.
struct Outcome {
    victim_loss: i128,
    attacker_gain: i128,
}

/// LP seul, 1 000 USDC dont 500 réservés. Il retire tout sauf 1 stroop de part (refusé après
/// B6, d'où le `try_`) ; une victime dépose 999 USDC ; la couverture se clôt à P&L nul ; tout
/// le monde sort.
fn inflation_by_exit_discount(n: &Net) -> Outcome {
    let attacker = n.junior(1_000 * USDC);
    let sme = n.open(500 * USDC);
    let first = n.c().try_withdraw(&attacker, &(1_000 * USDC - 1));
    if let Ok(Ok(_)) = first {
        // Aujourd'hui : 1 stroop de part vaut 5 000 000 001 stroops (≈ 500 USDC).
        assert_eq!(n.c().lp_shares(&attacker), 1);
    } else {
        assert_eq!(first, Err(Ok(code(LAST_SHARE_LOCKED))));
    }
    let victim = n.lp();
    n.deposit(&victim, 999 * USDC);
    n.close_flat(&sme, 500 * USDC);
    n.check();
    let victim_back = n.exit_junior(&victim);
    n.exit_junior(&attacker);
    n.check();
    Outcome { victim_loss: 999 * USDC - victim_back, attacker_gain: n.usdc_of(&attacker) - 1_000 * USDC }
}

/// ROUGE (B6) : après le plancher de parts, le retrait de l'attaquant est refusé (116) et la
/// victime récupère son dépôt à 1 stroop près.
#[test]
fn inflation_by_exit_discount_victim_loses_at_most_one_stroop() {
    let n = net();
    let o = inflation_by_exit_discount(&n);
    assert!(o.victim_loss <= 1, "perte de la victime : {}", o.victim_loss);
    assert!(o.attacker_gain <= 0, "gain de l'attaquant : {}", o.attacker_gain);
}

/// Variante à deux adresses : l'adresse A apporte 1 000 USDC et sort en entier, l'adresse B
/// (même attaquant) garde 1 stroop de part déposé avant.
fn inflation_two_addresses(n: &Net) -> Outcome {
    let a = n.junior(1_000 * USDC);
    let b = n.junior(1);
    let sme = n.open(500 * USDC);
    let first = n.c().try_withdraw(&a, &(1_000 * USDC));
    if !matches!(first, Ok(Ok(_))) {
        assert_eq!(first, Err(Ok(code(LAST_SHARE_LOCKED))));
    }
    let victim = n.lp();
    n.deposit(&victim, 999 * USDC);
    n.close_flat(&sme, 500 * USDC);
    let victim_back = n.exit_junior(&victim);
    for lp in [&a, &b] {
        if n.c().lp_shares(lp) > 0 {
            n.exit_junior(lp);
        }
    }
    n.check();
    Outcome {
        victim_loss: 999 * USDC - victim_back,
        attacker_gain: n.usdc_of(&a) + n.usdc_of(&b) - (1_000 * USDC + 1),
    }
}

/// ROUGE (B6) : le plancher porte sur le total des parts, pas sur l'adresse.
#[test]
fn inflation_two_addresses_victim_loses_at_most_one_stroop() {
    let n = net();
    let o = inflation_two_addresses(&n);
    assert!(o.victim_loss <= 1, "perte de la victime : {}", o.victim_loss);
}

/// Variante par un gain du pool : l'attaquant sort pendant qu'une couverture de 100 USDC est
/// ouverte (1 stroop de part reste, 100 USDC de capital), la couverture rapporte toute sa marge,
/// puis une victime dépose 399 USDC une fois Reserved retombé à 0.
fn inflation_by_pool_gain(n: &Net) -> Outcome {
    let attacker = n.junior(1_000 * USDC);
    let sme = n.open(100 * USDC);
    let first = n.c().try_withdraw(&attacker, &(1_000 * USDC - 1));
    if !matches!(first, Ok(Ok(_))) {
        assert_eq!(first, Err(Ok(code(LAST_SHARE_LOCKED))));
    }
    n.close_pool_wins(&sme, 100 * USDC, 100 * USDC);
    let victim = n.lp();
    n.deposit(&victim, 399 * USDC);
    let victim_back = n.exit_junior(&victim);
    n.exit_junior(&attacker);
    n.check();
    // Le gain légitime de la couverture (100 USDC) revient à l'attaquant, seul LP : on le retire.
    Outcome {
        victim_loss: 399 * USDC - victim_back,
        attacker_gain: n.usdc_of(&attacker) - 1_000 * USDC - 100 * USDC,
    }
}

/// ROUGE (B6).
#[test]
fn inflation_by_pool_gain_victim_loses_at_most_one_stroop() {
    let n = net();
    let o = inflation_by_pool_gain(&n);
    assert!(o.victim_loss <= 1, "perte de la victime : {}", o.victim_loss);
}

/// VERT : ce que B6 laisse passer. L'attaquant descend exactement au plancher (1 USDC de parts)
/// avec 500 USDC réservés : la part vaut 500,5 USDC. La victime ne perd plus que l'arrondi,
/// borné par la valeur d'un stroop de part (≈ 500 stroops), pas par 1 stroop.
#[test]
fn inflation_down_to_floor_only_costs_rounding() {
    let n = net();
    let attacker = n.junior(1_000 * USDC);
    let sme = n.open(500 * USDC);
    assert_eq!(n.c().withdraw(&attacker, &(1_000 * USDC - USDC)), 4_995_000_000);
    assert_eq!(n.c().share_price(), 5_005_000_000);
    let victim = n.lp();
    assert_eq!(n.deposit(&victim, 999 * USDC), 19_960_039);
    n.close_flat(&sme, 500 * USDC);
    let victim_back = n.exit_junior(&victim);
    let loss = 999 * USDC - victim_back;
    // 161 stroops : moins que la valeur d'un stroop de part (14 995 000 000 / 29 960 039 ≈ 500).
    assert_eq!(loss, 161);
    n.check();
}

// ---------------------------------------------------------------------------------------------
// Arrondis : dépôts minuscules répétés, allers-retours
// ---------------------------------------------------------------------------------------------

/// VERT : 100 dépôts de 10 stroops à 1,1 USDC la part donnent 9 parts chacun ; l'attaquant
/// paie l'arrondi, les LP en place l'encaissent.
#[test]
fn tiny_repeated_deposits_cost_the_attacker() {
    let n = net();
    let honest = n.junior(1_000 * USDC);
    let sme = n.open(100 * USDC);
    n.close_pool_wins(&sme, 100 * USDC, 100 * USDC);
    let attacker = n.lp();
    for _ in 0..100 {
        assert_eq!(n.deposit(&attacker, 10), 9);
    }
    // 900 parts × 1 100 / 1 000 = 990 : 10 stroops perdus sur 1 000.
    assert_eq!(n.exit_junior(&attacker), 990);
    assert_eq!(n.exit_junior(&honest), 1_100 * USDC + 10);
    n.check();
}

/// VERT : 1 000 retraits d'une part à un prix de sortie de 1,5 : 1 stroop versé par part au
/// lieu de 1,5. L'attaquant laisse 500 stroops aux LP restants.
#[test]
fn tiny_repeated_withdrawals_cost_the_attacker() {
    let n = net();
    let honest = n.junior(1_000 * USDC);
    let attacker = n.junior(1_000);
    let sme = n.open(500 * USDC + 500);
    n.close_pool_wins(&sme, 500 * USDC + 500, 500 * USDC + 500);
    assert_eq!(n.c().share_price(), 15_000_000);
    let mut got = 0;
    for _ in 0..1_000 {
        got += n.c().withdraw(&attacker, &1);
    }
    assert_eq!(got, 1_000);
    assert_eq!(n.exit_junior(&honest), 1_500 * USDC + 500);
    n.check();
}

/// VERT : un aller-retour dépôt / retrait ne rapporte jamais rien, même à un prix qui tombe mal
/// (2 000 USDC moins 1 stroop pour 3 000 parts) et avec une couverture ouverte.
#[test]
fn deposit_withdraw_round_trip_never_profits() {
    let n = net();
    n.junior(3_000 * USDC);
    let sme = n.open(1_000 * USDC);
    n.close_pool_loses(&sme, 1_000 * USDC, 1_000 * USDC + 1);
    n.open(7 * USDC);
    let attacker = n.lp();
    let mut total_in = 0;
    let mut total_out = 0;
    for amount in [1i128, 2, 3, 7, 10, 333, 1_000_001, 77 * USDC] {
        n.mint(&attacker, amount);
        if let Ok(Ok(_)) = n.c().try_deposit_lp(&attacker, &amount) {
            total_in += amount;
            let shares = n.c().lp_shares(&attacker);
            if let Ok(Ok(out)) = n.c().try_withdraw(&attacker, &shares) {
                total_out += out;
            }
        }
        assert!(total_out <= total_in, "aller-retour gagnant : {total_out} > {total_in}");
    }
    n.check();
}

/// VERT : aller-retour senior immédiat, au prix capitalisé : jamais plus que le dépôt.
#[test]
fn senior_round_trip_never_profits() {
    let n = net();
    n.junior(400_000 * USDC);
    n.senior(800_000 * USDC);
    n.advance(YEAR / 3 + 17);
    let attacker = n.lp();
    for amount in [1i128, 3, 1_045, 10_000_001, 123_456 * USDC] {
        n.mint(&attacker, amount);
        let before = n.usdc_of(&attacker);
        if let Ok(Ok(shares)) = n.c().try_deposit_senior(&attacker, &amount) {
            if let Ok(Ok(_)) = n.c().try_withdraw_senior(&attacker, &shares) {}
        }
        assert!(n.usdc_of(&attacker) <= before);
    }
    n.check();
}

// ---------------------------------------------------------------------------------------------
// Défaut 2 (B5) : intérêts senior qui font passer le junior sous Reserved
// ---------------------------------------------------------------------------------------------

/// Pool 200 000 junior / 800 000 senior, 180 000 réservés (et non 190 000 : avec la provision
/// d'intérêts de B5, capacité = 200 000 − 800 000 × 1,479 % ≈ 188 164, une réserve de 190 000 serait
/// refusée et le scénario ne se monterait plus). Un an plus tard : créance senior 836 000, junior
/// 164 000 < R. Retourne (senior, PME, ce que le senior reçoit en retirant tout).
fn senior_runs_under_cap(n: &Net) -> (Address, Address, i128) {
    n.junior(200_000 * USDC);
    let senior = n.senior(800_000 * USDC);
    let sme = n.open(180_000 * USDC);
    n.advance(YEAR);
    assert_eq!(n.c().tranche_values().senior_price, 10_450_000);
    let paid = n.c().withdraw_senior(&senior, &(800_000 * USDC));
    n.check();
    (senior, sme, paid)
}

/// ROUGE (B5) : le retrait est limité au payable, min(836 000, T − R = 820 000) ; T = R après
/// le retrait et le règlement perdant passe. Le senior garde une créance de 16 000 non servie.
#[test]
fn senior_exit_under_cap_is_limited_and_settle_passes() {
    let n = net();
    let (senior, sme, paid) = senior_runs_under_cap(&n);
    assert_eq!(paid, 820_000 * USDC);
    assert!(n.t() >= n.r(), "T = {} < R = {}", n.t(), n.r());
    n.close_pool_loses(&sme, 180_000 * USDC, 180_000 * USDC);
    n.check();
    assert_eq!(n.t(), 0);
    assert_eq!(n.usdc_of(&senior), 820_000 * USDC);
    assert_eq!(n.usdc_of(&sme), 360_000 * USDC);
}

/// ROUGE (B5) : retrait de la moitié des parts sous plafond : la moitié du payable (410 000).
#[test]
fn half_senior_exit_under_cap_gets_half_of_payable() {
    let n = net();
    n.junior(200_000 * USDC);
    let senior = n.senior(800_000 * USDC);
    n.open(180_000 * USDC);
    n.advance(YEAR);
    assert_eq!(n.c().senior_balance(&senior), 820_000 * USDC);
    assert_eq!(n.c().withdraw_senior(&senior, &(400_000 * USDC)), 410_000 * USDC);
    assert_eq!(n.t() - n.r(), 410_000 * USDC);
    n.check();
}

// ---------------------------------------------------------------------------------------------
// Défaut 3 (garde de B5) : dépôt junior sous plafond au juste prix
// ---------------------------------------------------------------------------------------------

/// VERT, à garder vert après B5 : même pool sous plafond (junior 164 000 < R = 180 000). Un dépôt
/// junior de 100 000 doit être émis sur la créance senior non plafonnée (836 000). Avec la
/// formule « naïve » min(couru, T − R), il recevrait 111 111 parts au lieu de 121 951 et
/// perdrait environ 5 700 USDC (9 000 environ avec R = 190 000) une fois la couverture close.
#[test]
fn junior_deposit_under_cap_is_priced_on_uncapped_claim() {
    let n = net();
    n.junior(200_000 * USDC);
    n.senior(800_000 * USDC);
    let sme = n.open(180_000 * USDC);
    n.advance(YEAR);
    let victim = n.lp();
    assert_eq!(n.deposit(&victim, 100_000 * USDC), 1_219_512_195_121);
    n.close_flat(&sme, 180_000 * USDC);
    // Couverture close : R = 0, le plafond ne joue plus, toutes les formules s'accordent. Valeur
    // au prix plein (junior = T − créance) : `lp_balance` passerait par la capacité, que la
    // provision d'intérêts de B5 abaisse.
    let value = mul_div(&n.env, n.c().lp_shares(&victim), n.c().tranche_values().junior, n.v(total_shares));
    assert!(100_000 * USDC - value <= 1, "perte du déposant junior : {}", 100_000 * USDC - value);
    n.check();
}

// ---------------------------------------------------------------------------------------------
// Senior : timing, dilution, capitalisation
// ---------------------------------------------------------------------------------------------

/// VERT : ouvrir la plus grosse couverture possible ne change pas ce que le senior peut retirer
/// (`reserve` exige une capacité suffisante). Personne ne gagne à retirer juste avant.
#[test]
fn senior_value_unchanged_by_biggest_open() {
    let n = net();
    n.junior(300_000 * USDC);
    let senior = n.senior(800_000 * USDC);
    n.advance(YEAR / 2);
    let before = n.c().senior_balance(&senior);
    assert_eq!(before, 818_000 * USDC);
    let cap = n.c().capacity();
    n.open(cap);
    assert_eq!(n.c().capacity(), 0);
    assert_eq!(n.c().senior_balance(&senior), before);
    assert_eq!(n.c().withdraw_senior(&senior, &(800_000 * USDC)), before);
    assert!(n.t() >= n.r());
    n.check();
}

/// VERT, à garder vert après B5 : un dépôt senior quand le payable est plafonné (junior 268 500 <
/// R = 280 000, ratio encore à 24 %) est soit refusé (118), soit servi au juste prix. Sans le 118,
/// B5 ferait perdre environ 1 380 USDC au nouveau senior au profit des anciens (payable 820 000
/// partagé sur 795 694 parts dont 95 694 achetées au prix couru 1,045).
#[test]
fn senior_deposit_under_cap_does_not_dilute() {
    let n = net();
    n.junior(300_000 * USDC);
    let old = n.senior(700_000 * USDC);
    n.open(280_000 * USDC);
    n.advance(YEAR);
    let old_before = n.c().senior_balance(&old);
    let newcomer = n.lp();
    n.mint(&newcomer, 100_000 * USDC);
    match n.c().try_deposit_senior(&newcomer, &(100_000 * USDC)) {
        Err(e) => assert_eq!(e, Ok(code(SENIOR_ARREARS))),
        Ok(_) => {
            let value = n.c().senior_balance(&newcomer);
            assert!(100_000 * USDC - value <= 2, "perte du nouveau senior : {}", 100_000 * USDC - value);
            assert!(n.c().senior_balance(&old) - old_before <= 1);
        }
    }
    n.check();
}

/// VERT (tolérance) : 365 retraits d'une part senior, un par jour, capitalisent le prix chaque
/// jour : 1,045 simple devient (1 + 4,5 %/365)^365 ≈ 1,046025. Borne théorique (capitalisation
/// continue) : e^0,045 − 1,045 ≈ 0,103 % du principal par an, payés par le junior.
#[test]
fn daily_one_share_withdrawals_compound_senior_price_within_bound() {
    let control = net();
    control.junior(250_000 * USDC);
    let control_senior = control.senior(800_000 * USDC);
    control.advance(YEAR);
    let simple = control.c().senior_balance(&control_senior);
    assert_eq!(simple, 836_000 * USDC);

    let n = net();
    n.junior(250_000 * USDC);
    let senior = n.senior(800_000 * USDC);
    let poker = n.lp();
    n.mint(&poker, 1_000);
    n.c().deposit_senior(&poker, &1_000);
    for _ in 0..365 {
        n.advance(DAY);
        assert_eq!(n.c().withdraw_senior(&poker, &1), 1);
    }
    let compounded = n.c().senior_balance(&senior);
    let drift = compounded - simple;
    assert_eq!(drift, 8_199_667_987); // 819,97 USDC
    assert!(drift <= 800_000 * USDC * 11 / 10_000, "dérive au-delà de 0,11 % : {drift}");
    n.check();
}

// ---------------------------------------------------------------------------------------------
// Ratio de 20 %, pool vidé
// ---------------------------------------------------------------------------------------------

/// VERT : junior 250 000 / senior 800 000. Quel que soit le nombre de parts demandé, un retrait
/// junior accepté laisse le junior à 20 % au moins ; tout retirer est refusé (111). Aujourd'hui la
/// borne tombe pile à 50 000 parts (prix de sortie 1) ; avec la provision d'intérêts de B5, le
/// prix de sortie baisse et la borne bouge : le test ne fige que la propriété.
#[test]
fn junior_cannot_push_ratio_below_20_pct() {
    let n = net();
    let junior = n.junior(250_000 * USDC);
    n.senior(800_000 * USDC);
    assert_eq!(n.c().try_withdraw(&junior, &(250_000 * USDC)), Err(Ok(code(111))));
    let mut accepted = 0;
    for shares in [70_000 * USDC, 62_500 * USDC + 1, 50_000 * USDC + 1, 50_000 * USDC, 1_000 * USDC, 1] {
        if let Ok(Ok(_)) = n.c().try_withdraw(&junior, &shares) {
            accepted += 1;
            let t = n.c().tranche_values();
            assert!(t.junior * BPS >= (t.junior + t.senior) * MIN_JUNIOR_BPS, "ratio sous 20 % : {t:?}");
        }
    }
    assert!(accepted > 0);
    let t = n.c().tranche_values();
    // Code actuel : 50 000 acceptés, puis plus rien ; junior à 20 % pile.
    assert!(t.junior >= 200_000 * USDC);
    n.check();
}

/// VERT (limite connue, recapitalisation = FUTUR) : une perte vide le junior, le pool n'accepte
/// plus aucun capital : dépôt junior 104, retrait du dernier LP 107. Les 100 parts restent à vie.
#[test]
fn depleted_pool_cannot_be_recapitalised() {
    let n = net();
    let lp = n.junior(100 * USDC);
    let sme = n.open(100 * USDC);
    n.close_pool_loses(&sme, 100 * USDC, 100 * USDC);
    assert_eq!(n.t(), 0);
    let rescuer = n.lp();
    n.mint(&rescuer, 1_000 * USDC);
    assert_eq!(n.c().try_deposit_lp(&rescuer, &(1_000 * USDC)), Err(Ok(code(104))));
    assert_eq!(n.c().try_withdraw(&lp, &(100 * USDC)), Err(Ok(code(107))));
    assert_eq!(n.c().lp_shares(&lp), 100 * USDC);
    n.check();
}

/// VERT (limite connue) : sans nouvelle perte, les seuls intérêts senior vident un junior mince
/// et ferment le pool : junior 20 000 après une perte de 180 000 ; un an plus tard la créance
/// (836 000) dépasse T (820 000). Dépôt junior 104, dépôt senior 111 ; le senior sort avec tout T.
#[test]
fn senior_interest_alone_depletes_thin_junior_and_locks_pool() {
    let n = net();
    let junior = n.junior(200_000 * USDC);
    let senior = n.senior(800_000 * USDC);
    let sme = n.open(180_000 * USDC);
    n.close_pool_loses(&sme, 180_000 * USDC, 180_000 * USDC);
    n.advance(YEAR);
    let t = n.c().tranche_values();
    assert_eq!((t.senior, t.junior), (820_000 * USDC, 0));
    let rescuer = n.lp();
    n.mint(&rescuer, 200_000 * USDC);
    assert_eq!(n.c().try_deposit_lp(&rescuer, &(100_000 * USDC)), Err(Ok(code(104))));
    assert_eq!(n.c().try_deposit_senior(&rescuer, &(100_000 * USDC)), Err(Ok(code(111))));
    assert_eq!(n.c().lp_balance(&junior), 0);
    assert_eq!(n.c().withdraw_senior(&senior, &(800_000 * USDC)), 820_000 * USDC);
    n.check();
}

// ---------------------------------------------------------------------------------------------
// Règlement opportuniste (limite 4c, lot B3) : de bout en bout, par open_hedge / settle
// ---------------------------------------------------------------------------------------------

/// Pool réel avec oracle : MXN à 0,055, taux USD 4 % / MXN 7 %, spread 0,5 %, plafond minimum
/// 5 % (comme `test.rs`). Un LP honnête apporte 100 000 USDC, Lucía ouvre l'achat de 1 000 000
/// MXN à 60 s, plafond 10 % (marge 5 527,4996845). L'échéance passe avec un MXN à −8 % : le pool
/// gagne 4 674,9968451 USDC, mais personne n'a encore appelé `settle`.
/// Retourne (Net, LP honnête, id de la couverture).
fn matured_winning_hedge() -> (Net, Address, u64) {
    use mock_oracle::{Asset, MockOracleClient};
    use soroban_sdk::Symbol;
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let admin = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let oracle_id = env.register(mock_oracle::MockOracle, (&admin,));
    let oracle = MockOracleClient::new(&env, &oracle_id);
    let mxn = Symbol::new(&env, "MXN");
    oracle.set_price(&Asset::Other(mxn.clone()), &5_500_000_000_000);
    let pool = env.register(Pool, (&admin, &usdc, &oracle_id, &50u32));
    let n = Net { env, pool, usdc };
    n.c().set_rate(&Symbol::new(&n.env, "USD"), &400);
    n.c().set_rate(&mxn, &700);
    n.c().set_min_cap(&mxn, &500);

    let honest = n.junior(100_000 * USDC);
    let lucia = Address::generate(&n.env);
    n.c().set_allowed(&Role::Sme, &lucia, &true);
    n.mint(&lucia, 200_000 * USDC);
    let supplier = Address::generate(&n.env);
    let id = n.c().open_hedge(&lucia, &supplier, &mxn, &Side::BuyCurrency, &10_000_000_000_000, &60, &1000, &None);
    n.advance(60);
    oracle.set_price(&Asset::Other(mxn), &5_060_000_000_000);
    (n, honest, id)
}

/// CONSTAT (à supprimer avec B3) : l'attaquant voit l'échéance passée et le cours, dépose
/// 100 000 à 1:1 (le gain n'est pas encore compté), appelle lui-même `settle`, puis retire
/// tout : il capte la moitié du gain, pris au LP honnête.
#[test]
fn constat_deposit_between_maturity_and_settle_captures_half_the_gain() {
    let (n, honest, id) = matured_winning_hedge();
    let attacker = n.lp();
    assert_eq!(n.deposit(&attacker, 100_000 * USDC), 100_000 * USDC);
    n.c().settle(&id);
    assert_eq!(n.t(), 200_000 * USDC + 46_749_968_451);
    let attacker_gain = n.exit_junior(&attacker) - 100_000 * USDC;
    assert_eq!(attacker_gain, 23_374_984_225);
    // Le LP honnête n'a plus que la moitié du gain de sa propre couverture.
    assert_eq!(n.c().lp_balance(&honest), 100_000 * USDC + 23_374_984_226);
    n.check();
}

/// FUTUR (B3) : le dépôt est refusé tant que la couverture échue n'est pas réglée, accepté
/// juste après ; le LP honnête garde tout le gain.
#[test]
#[ignore = "FUTUR : lot B3 (dépôts refusés tant qu'une couverture échue n'est pas réglée)"]
fn deposit_refused_between_maturity_and_settle() {
    let (n, honest, id) = matured_winning_hedge();
    let attacker = n.lp();
    n.mint(&attacker, 100_000 * USDC);
    assert_eq!(n.c().try_deposit_lp(&attacker, &(100_000 * USDC)), Err(Ok(code(MATURED_HEDGE_PENDING))));
    assert_eq!(n.c().try_deposit_senior(&attacker, &(100_000 * USDC)), Err(Ok(code(MATURED_HEDGE_PENDING))));
    n.c().settle(&id);
    assert_eq!(n.c().lp_balance(&honest), 100_000 * USDC + 46_749_968_451);
    // Après le règlement, le dépôt passe au nouveau prix et ne capte rien.
    n.c().deposit_lp(&attacker, &(100_000 * USDC));
    assert!(100_000 * USDC - n.exit_junior(&attacker) <= 1);
    assert_eq!(n.c().lp_balance(&honest), 100_000 * USDC + 46_749_968_451);
    n.check();
}
