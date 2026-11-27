// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
//! Filet de tests « monde » du vault : uniquement les points d'entrée publics du pool
//! (`deposit_lp`, `deposit_senior`, `open_hedge`, `settle`, `close_early`, `withdraw`,
//! `withdraw_senior`, lectures), le vrai mock-oracle (prix horodatés) et la vraie mécanique de
//! cours d'échéance de B1. Aucun `as_contract`, aucun appel direct à `reserve` / `book_*`.
//!
//! Convention : VERT = passe et doit continuer de passer ; ROUGE = `#[ignore]` jusqu'au lot qui
//! corrige le défaut ; FUTUR = `#[cfg(any())]` (jamais compilé) jusqu'à ce que le lot existe.
extern crate std;

use crate::{hedge, Pool, PoolClient, Role, Side, PRICE_DECIMALS};
use mock_oracle::{Asset as OracleAsset, MockOracle, MockOracleClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env, Symbol};
use std::cell::RefCell;
use std::vec::Vec;

/// 1 USDC (7 décimales).
const USDC: i128 = 10_000_000;
/// 1 unité de devise (7 décimales).
const UNIT: i128 = 10_000_000;
/// Prix 14 décimales : `px(5, 2)` = 0,05 USD.
const fn px(units: i128, decimals: u32) -> i128 {
    units * 10i128.pow(PRICE_DECIMALS - decimals)
}
const T0: u64 = 1_000_000;
const DAY: u64 = 86_400;
const YEAR: u64 = 31_536_000;
/// Plancher de parts junior prévu par B6 (remplacer par `super::MIN_JUNIOR_SHARES` quand il existe).
const JUNIOR_FLOOR: i128 = 10_000_000;

// ---- codes d'erreur attendus ----
const E_LP_NOT_ALLOWED: u32 = 114;
const E_NOTIONAL_CAP: u32 = 115;
const E_LAST_SHARE_LOCKED: u32 = 116; // B6
const E_PAUSED: u32 = 400;
const E_NO_FIXING: u32 = 312;

type TryResult<T, E> = Result<Result<T, E>, Result<soroban_sdk::Error, soroban_sdk::InvokeError>>;

/// Vérifie qu'un appel `try_*` a échoué avec le code de contrat `code`.
fn assert_err<T: core::fmt::Debug, E: core::fmt::Debug>(r: TryResult<T, E>, code: u32) {
    match r {
        Err(Ok(e)) => assert_eq!(e, soroban_sdk::Error::from_contract_error(code), "code attendu {code}"),
        other => panic!("attendu le code {code}, obtenu {other:?}"),
    }
}

// ---- le monde ----

struct World<'a> {
    env: Env,
    guardian: Address,
    usdc: TokenClient<'a>,
    usdc_admin: StellarAssetClient<'a>,
    oracle: MockOracleClient<'a>,
    pool: PoolClient<'a>,
    /// (devise, dernier prix publié) : republié à chaque `goto` pour que l'oracle reste frais.
    prices: RefCell<Vec<(Symbol, i128)>>,
    juniors: RefCell<Vec<Address>>,
    seniors: RefCell<Vec<Address>>,
    hedges: RefCell<Vec<u64>>,
}

/// Pool sans spread ni taux (taux bloqué = spot, montants ronds), garde posée, horloge à `T0`.
/// Point unique à adapter quand la moitié hedge de B9 posera des limites par défaut (plafond par
/// couverture, durée maximale, liste PME) : les lever ici.
fn world<'a>() -> World<'a> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(T0);
    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let usdc_id = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let oracle_id = env.register(MockOracle, (&admin,));
    let pool_id = env.register(Pool, (&admin, &usdc_id, &oracle_id, 0u32));
    PoolClient::new(&env, &pool_id).set_senior_deposits_open(&true); // fermés par défaut (B5)
    let pool = PoolClient::new(&env, &pool_id);
    pool.set_rate(&Symbol::new(&env, "USD"), &0);
    pool.set_guardian(&guardian);
    World {
        usdc: TokenClient::new(&env, &usdc_id),
        usdc_admin: StellarAssetClient::new(&env, &usdc_id),
        oracle: MockOracleClient::new(&env, &oracle_id),
        pool,
        guardian,
        prices: RefCell::new(Vec::new()),
        juniors: RefCell::new(Vec::new()),
        seniors: RefCell::new(Vec::new()),
        hedges: RefCell::new(Vec::new()),
        env,
    }
}

/// Monde avec MXN à 0,05 USD (taux 0, plafond minimum 5 %).
fn world_mxn<'a>() -> World<'a> {
    let w = world();
    w.add_currency("MXN", px(5, 2), 0);
    w
}

impl<'a> World<'a> {
    fn sym(&self, code: &str) -> Symbol {
        Symbol::new(&self.env, code)
    }

    fn now(&self) -> u64 {
        self.env.ledger().timestamp()
    }

    /// Ouvre une devise : taux annuel, plafond minimum 5 %, premier prix publié maintenant.
    fn add_currency(&self, code: &str, price: i128, rate_bps: u32) {
        let c = self.sym(code);
        self.pool.set_rate(&c, &rate_bps);
        self.pool.set_min_cap(&c, &500);
        self.prices.borrow_mut().push((c.clone(), price));
        self.publish(&c, price);
    }

    fn publish(&self, c: &Symbol, price: i128) {
        self.oracle.set_price(&OracleAsset::Other(c.clone()), &price);
    }

    /// Change le cours d'une devise et le publie maintenant (horodaté au ledger courant).
    fn set_spot(&self, code: &str, price: i128) {
        let c = self.sym(code);
        for p in self.prices.borrow_mut().iter_mut() {
            if p.0 == c {
                p.1 = price;
            }
        }
        self.publish(&c, price);
    }

    /// Avance l'horloge SANS publier de prix (pour fabriquer un cours absent ou périmé).
    fn warp(&self, ts: u64) {
        assert!(ts >= self.now(), "le temps ne recule pas");
        self.env.ledger().set_timestamp(ts);
    }

    /// Avance l'horloge et republie le dernier prix de chaque devise : l'oracle reste frais
    /// (ouvertures possibles) et B1 trouve un cours en vigueur à toute échéance tombant sur `ts`.
    /// Attention B1 : sauter plus de 900 s au-delà d'une échéance non réglée sans publier à
    /// l'échéance la rend réglable seulement par le premier cours de l'heure qui suit.
    fn goto(&self, ts: u64) {
        self.warp(ts);
        let prices = self.prices.borrow().clone();
        for (c, p) in prices.iter() {
            self.publish(c, *p);
        }
    }

    fn advance(&self, secs: u64) {
        self.goto(self.now() + secs);
    }

    fn mint(&self, to: &Address, amount: i128) {
        self.usdc_admin.mint(to, &amount);
    }

    fn balance(&self, who: &Address) -> i128 {
        self.usdc.balance(who)
    }

    /// LP junior autorisé, financé, qui dépose `amount`.
    fn junior(&self, amount: i128) -> Address {
        let lp = Address::generate(&self.env);
        self.pool.set_allowed(&Role::Lp, &lp, &true);
        self.mint(&lp, amount);
        self.pool.deposit_lp(&lp, &amount);
        self.juniors.borrow_mut().push(lp.clone());
        lp
    }

    /// LP senior autorisé, financé, qui dépose `amount`. Retourne (adresse, parts senior) : aucune
    /// lecture publique ne donne les parts senior d'un LP, seul `deposit_senior` les renvoie.
    /// (B5 : ouvrir d'abord les dépôts Safe ici.)
    fn senior(&self, amount: i128) -> (Address, i128) {
        let lp = Address::generate(&self.env);
        self.pool.set_allowed(&Role::Lp, &lp, &true);
        self.mint(&lp, amount);
        let shares = self.pool.deposit_senior(&lp, &amount);
        self.seniors.borrow_mut().push(lp.clone());
        (lp, shares)
    }

    /// PME avec `usdc` USDC pour ses marges.
    fn sme(&self, usdc: i128) -> Address {
        let sme = Address::generate(&self.env);
        self.pool.set_allowed(&Role::Sme, &sme, &true); // liste blanche PME active par défaut
        self.mint(&sme, usdc);
        sme
    }

    fn open(&self, sme: &Address, code: &str, side: Side, notional: i128, duration: u64, cap_bps: u32) -> u64 {
        let supplier = Address::generate(&self.env);
        let id = self.pool.open_hedge(sme, &supplier, &self.sym(code), &side, &notional, &duration, &cap_bps, &None);
        self.hedges.borrow_mut().push(id);
        id
    }

    /// Va à l'échéance (si elle n'est pas passée), publie `price` comme cours d'échéance et règle.
    /// Exige le succès (jamais de 103). B4 : appeler ensuite `claim_for(owner)` ici.
    fn settle_at(&self, id: u64, code: &str, price: i128) -> i128 {
        let h = self.pool.get_hedge(&id).unwrap();
        if self.now() < h.maturity {
            self.goto(h.maturity);
        }
        self.set_spot(code, price);
        self.pool.settle(&id)
    }

    fn open_hedges(&self) -> Vec<crate::Hedge> {
        self.hedges
            .borrow()
            .iter()
            .map(|id| self.pool.get_hedge(id).unwrap())
            .filter(|h| !h.settled)
            .collect()
    }

    /// Propriétaires distincts des couvertures ouvertes par ce monde.
    fn owners(&self) -> Vec<Address> {
        let mut v: Vec<Address> = Vec::new();
        for id in self.hedges.borrow().iter() {
            let o = self.pool.get_hedge(id).unwrap().owner;
            if !v.contains(&o) {
                v.push(o);
            }
        }
        v
    }

    /// Σ soldes réclamables (B4), recalculé par propriétaire et recoupé avec `total_claimable`.
    fn claimable_total(&self) -> i128 {
        let sum: i128 = self.owners().iter().map(|o| self.pool.claimable(o)).sum();
        assert_eq!(sum, self.pool.total_claimable(), "Σ claimable(owner) = total_claimable");
        sum
    }

    /// Pousse tous les soldes réclamables vers leurs propriétaires (robot, `claim_for`).
    fn claim_all(&self) {
        for o in self.owners() {
            self.pool.claim_for(&o);
        }
    }

    /// Vérification d'état au repos, par les seules lectures publiques + le solde USDC du contrat.
    /// R est reconstitué par deux chemins indépendants (couvertures ouvertes, expositions).
    fn assert_state(&self) {
        let p = &self.pool;
        let t = p.total_assets();
        let tv = p.tranche_values();
        let open = self.open_hedges();
        let r: i128 = open.iter().map(|h| h.margin).sum();
        let scale = 10i128.pow(PRICE_DECIMALS);
        let open_notional: i128 = open.iter().map(|h| h.notional * h.locked_rate / scale).sum();

        // R par les expositions (lecture publique) = R par les couvertures ouvertes.
        let (mut exp_margin, mut exp_notional) = (0i128, 0i128);
        for (c, _) in self.prices.borrow().iter() {
            let e = p.exposure(c);
            exp_margin += e.buy_margin + e.sell_margin;
            exp_notional += e.buy_notional + e.sell_notional;
            assert_eq!(e.net_notional, e.buy_notional - e.sell_notional);
        }
        assert_eq!(exp_margin, r, "Σ marges des expositions = Σ marges des couvertures ouvertes");
        assert_eq!(exp_notional, open_notional, "Σ notionnels des expositions");
        assert_eq!(p.total_notional(), open_notional, "TotalNotional = Σ notionnels ouverts");

        // I2 : solde USDC = T + marges ouvertes (+ réclamables, B4).
        assert_eq!(
            self.balance(&p.address),
            t + r + self.claimable_total(),
            "I2 : solde USDC du contrat = T + R + réclamables"
        );
        // Tranches.
        assert_eq!(tv.senior + tv.junior, t, "senior + junior = T");
        assert!(tv.senior >= 0 && tv.junior >= 0);
        // I1.
        assert!(r >= 0 && t >= r, "I1 : T ≥ R ≥ 0 (T = {t}, R = {r})");
        // I3.
        let cap = p.capacity();
        assert!(cap >= 0, "I3 : capacité ≥ 0 (capacité = {cap})");
        assert!(cap <= tv.junior - r, "I3 : capacité ≤ junior − R");
        assert!(p.exit_price() <= p.share_price(), "I3 : prix de sortie ≤ prix plein");
        // I4.
        let seniors: i128 = self.seniors.borrow().iter().map(|lp| p.senior_balance(lp)).sum();
        assert!(seniors <= (t - r).max(0), "I4 : Σ senior_balance ({seniors}) ≤ T − R ({})", t - r);
        let juniors: i128 = self.juniors.borrow().iter().map(|lp| p.lp_balance(lp)).sum();
        assert!(juniors <= cap, "I4 : Σ lp_balance ≤ capacité");
        // I6 (B6) : R > 0 ⇒ parts junior ≥ plancher.
        let shares: i128 = self.juniors.borrow().iter().map(|lp| p.lp_shares(lp)).sum();
        assert!(r == 0 || shares >= JUNIOR_FLOOR, "I6 : R > 0 et parts junior ({shares}) sous le plancher");
    }
}

// =====================================================================================
// VERT
// =====================================================================================

/// Trois devises, vrais taux et spread, tranches, temps qui passe, échéances successives.
/// Chaque règlement bouge T de −P&L exactement ; assert_state après chaque pas ; I7 en passant.
#[test]
fn w_multi_currency_round_trip() {
    let w = world();
    w.pool.set_rate(&w.sym("USD"), &400);
    w.pool.set_spread(&50);
    w.add_currency("MXN", px(5, 2), 700);
    w.add_currency("EUR", px(110, 2), 250);
    w.add_currency("BRL", px(20, 2), 1000);
    let j1 = w.junior(200_000 * USDC);
    let _j2 = w.junior(100_000 * USDC);
    let (s1, s1_shares) = w.senior(600_000 * USDC);
    w.assert_state();

    let sme = w.sme(100_000 * USDC);
    let a = w.open(&sme, "MXN", Side::BuyCurrency, 1_000_000 * UNIT, 90 * DAY, 1000);
    let b = w.open(&sme, "EUR", Side::SellCurrency, 100_000 * UNIT, 30 * DAY, 500);
    let c = w.open(&sme, "BRL", Side::BuyCurrency, 500_000 * UNIT, 60 * DAY, 2000);
    w.assert_state();
    assert_eq!(w.pool.exposure(&w.sym("EUR")).net_notional, -w.pool.get_hedge(&b).unwrap().notional * w.pool.get_hedge(&b).unwrap().locked_rate / 10i128.pow(PRICE_DECIMALS));

    // I7 : dépôt puis retrait immédiat ne rend jamais plus que déposé (couvertures ouvertes).
    let probe = w.junior(10_000 * USDC);
    let got = w.pool.withdraw(&probe, &w.pool.lp_shares(&probe));
    assert!(got <= 10_000 * USDC, "I7 : {got}");
    w.assert_state();

    // EUR à 30 jours : l'euro baisse de 3 %, l'exportateur gagne (le pool paie).
    let settle = |id: u64, code: &str, price: i128| {
        let h = w.pool.get_hedge(&id).unwrap();
        let before = w.pool.total_assets();
        let sme_before = w.balance(&h.owner);
        let claim_before = w.pool.claimable(&h.owner);
        let paid = w.settle_at(id, code, price);
        let pnl = hedge::pnl(&w.env, h.side, h.notional, h.locked_rate, price, h.margin);
        assert_eq!(paid, h.margin + pnl);
        assert_eq!(w.pool.total_assets(), before - pnl, "T bouge de −P&L");
        assert_eq!(w.balance(&h.owner), sme_before, "B4 : rien n'est transféré au règlement");
        assert_eq!(w.pool.claimable(&h.owner), claim_before + paid, "B4 : versement crédité");
        w.assert_state();
        pnl
    };
    assert!(settle(b, "EUR", px(10670, 4)) > 0);
    // BRL à 60 jours : le réal s'effondre de 30 %, l'acheteur perd toute sa marge.
    assert_eq!(settle(c, "BRL", px(14, 2)), -w.pool.get_hedge(&c).unwrap().margin);
    // MXN à 90 jours : +15 %, perte du pool bornée à la marge.
    assert_eq!(settle(a, "MXN", px(575, 4)), w.pool.get_hedge(&a).unwrap().margin);

    // Tout est réglé : sorties complètes (senior d'abord, sinon le ratio de 20 % bloque le junior).
    assert!(w.pool.capacity() <= w.pool.tranche_values().junior); // = sans provision B5
    let owed = w.pool.senior_balance(&s1);
    assert_eq!(w.pool.withdraw_senior(&s1, &s1_shares), owed);
    for lp in w.juniors.borrow().iter() {
        let shares = w.pool.lp_shares(lp);
        if shares > 0 {
            w.pool.withdraw(lp, &shares);
        }
    }
    assert_eq!(w.pool.total_assets(), 0);
    w.claim_all();
    assert_eq!(w.balance(&w.pool.address), 0, "plus un stroop dans le contrat");
    let _ = j1;
}

/// B10 × vault : la pause bloque dépôts et ouvertures, jamais les sorties ni les règlements.
#[test]
fn w_pause_blocks_entries_not_exits() {
    let w = world_mxn();
    let j = w.junior(100_000 * USDC);
    let (s, s_shares) = w.senior(100_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let a = w.open(&sme, "MXN", Side::BuyCurrency, 100_000 * UNIT, 30 * DAY, 1000); // marge 500
    let b = w.open(&sme, "MXN", Side::SellCurrency, 100_000 * UNIT, 60 * DAY, 1000);

    w.pool.pause(&w.guardian);
    let lp = Address::generate(&w.env);
    w.pool.set_allowed(&Role::Lp, &lp, &true);
    w.mint(&lp, 1_000 * USDC);
    assert_err(w.pool.try_deposit_lp(&lp, &(1_000 * USDC)), E_PAUSED);
    assert_err(w.pool.try_deposit_senior(&lp, &(1_000 * USDC)), E_PAUSED);
    let supplier = Address::generate(&w.env);
    assert_err(
        w.pool.try_open_hedge(&sme, &supplier, &w.sym("MXN"), &Side::BuyCurrency, &(1_000 * UNIT), &DAY, &1000, &None),
        E_PAUSED,
    );

    // Sorties sous pause : clôture anticipée, règlement, retraits junior et senior.
    w.advance(10 * DAY);
    w.pool.close_early(&b);
    w.settle_at(a, "MXN", px(6, 2)); // +20 % : perte du pool = marge 500
    w.assert_state();
    let owed = w.pool.senior_balance(&s);
    assert_eq!(w.pool.withdraw_senior(&s, &s_shares), owed);
    w.pool.withdraw(&j, &w.pool.lp_shares(&j));
    w.assert_state();

    w.pool.unpause();
    w.pool.deposit_lp(&lp, &(1_000 * USDC));
}

/// B9 × vault : un LP retiré de la liste blanche ne dépose plus mais récupère tout.
#[test]
fn w_removed_lp_still_withdraws_everything() {
    let w = world_mxn();
    let j = w.junior(50_000 * USDC);
    let (s, s_shares) = w.senior(50_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 200_000 * UNIT, 7 * DAY, 1000); // marge 1 000
    w.settle_at(id, "MXN", px(4, 2)); // −20 % : le pool gagne la marge
    assert_eq!(w.pool.total_assets(), 101_000 * USDC);

    w.pool.set_allowed(&Role::Lp, &j, &false);
    w.pool.set_allowed(&Role::Lp, &s, &false);
    assert!(!w.pool.is_allowed(&Role::Lp, &j));
    w.mint(&j, USDC);
    assert_err(w.pool.try_deposit_lp(&j, &USDC), E_LP_NOT_ALLOWED);
    assert_err(w.pool.try_deposit_senior(&j, &USDC), E_LP_NOT_ALLOWED);

    let senior_paid = w.pool.withdraw_senior(&s, &s_shares);
    let junior_paid = w.pool.withdraw(&j, &w.pool.lp_shares(&j));
    // 7 jours d'intérêts senior à 4,5 % payés par le junior, gain de 1 000 au junior.
    assert_eq!(senior_paid + junior_paid, 101_000 * USDC);
    assert!(senior_paid > 50_000 * USDC && junior_paid > 50_000 * USDC);
    assert_eq!(w.balance(&w.pool.address), 0);
    w.assert_state();
}

/// B9 × vault : plafond du notionnel total à la borne, un stroop au-delà, retour à zéro.
#[test]
fn w_notional_cap_at_bound() {
    let w = world_mxn();
    w.junior(100_000 * USDC);
    w.pool.set_notional_cap(&Some(50_000 * USDC));
    let sme = w.sme(10_000 * USDC);
    // 1 000 000 MXN × 0,05 = 50 000 USDC pile : accepté.
    let a = w.open(&sme, "MXN", Side::BuyCurrency, 1_000_000 * UNIT, 30 * DAY, 1000);
    assert_eq!(w.pool.total_notional(), 50_000 * USDC);
    // 20 stroops de MXN = 1 stroop d'USDC (marge arrondie à 1) : 115.
    let supplier = Address::generate(&w.env);
    assert_err(
        w.pool.try_open_hedge(&sme, &supplier, &w.sym("MXN"), &Side::SellCurrency, &20, &DAY, &1000, &None),
        E_NOTIONAL_CAP,
    );
    w.assert_state();
    // Le plafond ne bloque jamais un règlement ; le compteur retombe à 0.
    w.pool.set_notional_cap(&Some(0));
    w.settle_at(a, "MXN", px(5, 2));
    assert_eq!(w.pool.total_notional(), 0);
    w.pool.set_notional_cap(&Some(50_000 * USDC));
    w.open(&sme, "MXN", Side::SellCurrency, 1_000_000 * UNIT, 30 * DAY, 1000);
    w.assert_state();
}

/// B1 : aucun cours à l'échéance, un cours publié 10 s après. La recherche avance par pas de
/// 300 s : 312 jusqu'à échéance + 300 s, puis réglé à ce cours.
#[test]
fn w_fixing_published_after_maturity_found_at_next_step() {
    let w = world_mxn();
    w.junior(100_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 100_000 * UNIT, 2 * 3_600, 1000); // marge 500
    let m = w.pool.get_hedge(&id).unwrap().maturity;
    w.warp(m); // dernier cours : T0, 7 200 s avant l'échéance (> 900 s)
    assert_err(w.pool.try_settle(&id), E_NO_FIXING);
    w.warp(m + 10);
    w.set_spot("MXN", px(55, 3)); // +10 %
    assert_err(w.pool.try_settle(&id), E_NO_FIXING); // pas encore à m + 300
    w.warp(m + 300);
    assert_eq!(w.pool.settle(&id), 1_000 * USDC); // marge 500 + P&L 500
    w.assert_state();
}

/// B1 × vault : couverture sans cours (312) au-delà de l'heure de grâce. Elle garde R > 0 ;
/// un cours tardif ne la débloque pas ; capacité, retrait partiel et retrait senior restent
/// cohérents (T ≥ R). Le sort du DERNIER LP est dans le test ROUGE associé.
#[test]
fn w_hedge_without_fixing_keeps_reserve() {
    let w = world_mxn();
    let j = w.junior(10_000 * USDC);
    let (s, s_shares) = w.senior(10_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 100_000 * UNIT, 2 * 3_600, 1000); // marge 500
    let m = w.pool.get_hedge(&id).unwrap().maturity;
    w.warp(m + 3_600 + 1);
    w.set_spot("MXN", px(6, 2)); // cours tardif, hors fenêtre
    assert_err(w.pool.try_settle(&id), E_NO_FIXING);
    assert_eq!(w.pool.exposure(&w.sym("MXN")).buy_margin, 500 * USDC);
    w.assert_state();

    // Retrait senior total : ne touche pas au junior, T reste ≥ R.
    w.pool.withdraw_senior(&s, &s_shares);
    w.assert_state();
    // Retrait junior de la moitié des parts : moitié du capital libre (junior − 500).
    let half = w.pool.lp_shares(&j) / 2;
    let free = w.pool.capacity();
    assert_eq!(w.pool.withdraw(&j, &half), free / 2);
    w.assert_state();
}

/// close_early libère la réserve, l'exposition et le notionnel, au taux de sortie du jour.
#[test]
fn w_close_early_releases_reserve() {
    let w = world_mxn();
    w.junior(100_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 1_000_000 * UNIT, 90 * DAY, 1000); // marge 5 000
    assert_eq!(w.pool.capacity(), 95_000 * USDC);
    w.advance(30 * DAY);
    w.set_spot("MXN", px(52, 3)); // +4 % : sans taux ni spread, taux de sortie = spot
    assert_eq!(w.pool.close_early(&id), 7_000 * USDC); // marge 5 000 + P&L 2 000
    assert_eq!(w.pool.capacity(), 98_000 * USDC);
    assert_eq!(w.pool.total_notional(), 0);
    assert_err(w.pool.try_settle(&id), 201); // déjà réglée
    w.assert_state();
}

/// Garde contre la formule « naïve » de B5 (min(couru, T − R) partout) : un dépôt junior fait
/// pendant que les intérêts senior dépassent T − R doit être émis sur la créance NON plafonnée.
/// Pool 200 000 / 800 000, R = 185 000, dépôt de 100 000 une heure avant l'échéance (avant
/// l'échéance pour rester valable avec B3). Mesure robuste à la provision de B5 : après règlement
/// à plat et sortie des seniors, le déposant retire tout.
#[test]
fn w_junior_entry_price_uses_uncapped_senior_claim() {
    let w = world_mxn();
    let j = w.junior(200_000 * USDC);
    let (s, s_shares) = w.senior(800_000 * USDC);
    let sme = w.sme(200_000 * USDC);
    // 37 000 000 MXN × 0,05 = 1 850 000 USDC, plafond 10 % : marge 185 000.
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 37_000_000 * UNIT, YEAR, 1000);
    w.goto(T0 + YEAR - 3_600);
    let victim = w.junior(100_000 * USDC);
    w.settle_at(id, "MXN", px(5, 2)); // à plat
    w.pool.withdraw_senior(&s, &s_shares);
    let got = w.pool.withdraw(&victim, &w.pool.lp_shares(&victim));
    std::println!("dépôt junior sous plafond : rendu {got}");
    assert!(got >= 99_998 * USDC, "déposant junior surpayé : rendu {got} pour 100 000");
    assert!(got <= 100_000 * USDC);
    w.pool.withdraw(&j, &w.pool.lp_shares(&j));
    w.assert_state();
}

// =====================================================================================
// ROUGE (doivent passer après B6 / B5)
// =====================================================================================

/// Défaut 1, de bout en bout : inflation par la décote de sortie.
/// LP seul, 1 000 USDC, couverture réelle de marge 500. Code actuel : il brûle toutes ses parts
/// sauf 1 stroop ; le déposant de 999 USDC reçoit 1 part et, après un règlement à plat, retire
/// 749,5 USDC : perte 249,5. Après B6 : 116, le LP garde le plancher, perte négligeable.
#[test]
fn r_inflation_by_exit_discount() {
    let w = world_mxn();
    let attacker = w.junior(1_000 * USDC);
    let sme = w.sme(1_000 * USDC);
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 100_000 * UNIT, 30 * DAY, 1000); // marge 500
    assert_eq!(w.pool.capacity(), 500 * USDC);

    let shares = w.pool.lp_shares(&attacker);
    match w.pool.try_withdraw(&attacker, &(shares - 1)) {
        Ok(_) => {} // code actuel : l'attaque passe
        Err(Ok(e)) => {
            assert_eq!(e, soroban_sdk::Error::from_contract_error(E_LAST_SHARE_LOCKED));
            assert_err(w.pool.try_withdraw(&attacker, &shares), E_LAST_SHARE_LOCKED);
            w.pool.withdraw(&attacker, &(shares - JUNIOR_FLOOR)); // laisser pile le plancher : accepté
        }
        other => panic!("{other:?}"),
    }
    let victim = w.junior(999 * USDC);
    std::println!("parts de la victime : {}", w.pool.lp_shares(&victim));
    w.settle_at(id, "MXN", px(5, 2)); // à plat : R = 0
    let got = w.pool.withdraw(&victim, &w.pool.lp_shares(&victim));
    assert!(999 * USDC - got <= 1_000, "victime : perte de {} stroops", 999 * USDC - got);
    // R = 0 : la sortie totale redevient permise.
    w.pool.withdraw(&attacker, &w.pool.lp_shares(&attacker));
    w.claim_all();
    assert_eq!(w.balance(&w.pool.address), 0);
}

/// Défaut 2, de bout en bout : intérêts senior qui font passer le junior sous R.
/// Pool 200 000 / 800 000, couverture réelle de marge 185 000 à un an. Au bout d'un an : créance
/// senior 836 000, junior 164 000 < R. Code actuel : capacité −21 000, `withdraw_senior` total
/// verse 836 000, T = 164 000 < R, et le règlement perdant à pleine marge échoue en 103.
/// Après B5 : capacité 0, retrait limité au payable 815 000, T = R = 185 000, règlement passé.
#[test]
fn r_senior_interest_then_withdraw_blocks_losing_settle() {
    let w = world_mxn();
    w.junior(200_000 * USDC);
    let (s, s_shares) = w.senior(800_000 * USDC);
    let sme = w.sme(200_000 * USDC);
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 37_000_000 * UNIT, YEAR, 1000);
    assert_eq!(w.pool.get_hedge(&id).unwrap().margin, 185_000 * USDC);
    w.goto(T0 + YEAR);
    assert_eq!(w.pool.tranche_values().senior, 836_000 * USDC, "créance senior (non plafonnée)");
    assert_eq!(w.pool.capacity(), 0, "capacité jamais négative");

    assert_eq!(w.pool.withdraw_senior(&s, &s_shares), 815_000 * USDC, "retrait = payable T − R");
    assert_eq!(w.pool.total_assets(), 185_000 * USDC);
    w.assert_state();
    w.set_spot("MXN", px(6, 2)); // +20 % : perte du pool = marge
    let r = w.pool.try_settle(&id);
    assert!(r.is_ok(), "règlement perdant après retrait senior : {r:?}");
    assert_eq!(w.pool.total_assets(), 0);
}

/// B1 × B6 : couverture sans cours (312, fenêtre écoulée) qui garde R > 0. Le dernier LP ne
/// peut pas brûler sa dernière part (116) ; il retire jusqu'au plancher. Code actuel : il brûle
/// tout, T = R = 500 sans aucune part, et le prochain déposant hérite de la couverture bloquée.
#[test]
fn r_last_lp_with_hedge_without_fixing() {
    let w = world_mxn();
    let j = w.junior(10_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 100_000 * UNIT, 2 * 3_600, 1000);
    let m = w.pool.get_hedge(&id).unwrap().maturity;
    w.warp(m + 3_600 + 1);
    assert_err(w.pool.try_settle(&id), E_NO_FIXING);
    let shares = w.pool.lp_shares(&j);
    assert_err(w.pool.try_withdraw(&j, &shares), E_LAST_SHARE_LOCKED);
    w.pool.withdraw(&j, &(shares - JUNIOR_FLOOR));
    w.assert_state();
}

// =====================================================================================
// FUTUR : `#[cfg(any())]` = jamais compilé. Remplacer par rien (garder `#[test]`) quand le lot
// existe. Noms de fonctions et codes : ceux des plans, à confirmer à l'écriture du lot.
// =====================================================================================

/// FUTUR B3 : dépôts refusés tant qu'une couverture échue n'est pas réglée ; retraits ouverts ;
/// acceptés dès le règlement. Code 113 (plan-vault ; plan-suite disait 114, pris par B9).
#[cfg(any())]
#[test]
fn f_b3_deposit_refused_while_matured_hedge_pending() {
    let w = world_mxn();
    let j = w.junior(100_000 * USDC);
    let lp = Address::generate(&w.env);
    w.pool.set_allowed(&Role::Lp, &lp, &true);
    w.mint(&lp, 10_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 100_000 * UNIT, 7 * DAY, 1000);
    let m = w.pool.get_hedge(&id).unwrap().maturity;
    w.goto(m - 1);
    w.pool.deposit_lp(&lp, &(1_000 * USDC)); // une seconde avant : accepté (limite connue)
    w.goto(m);
    assert_eq!(w.pool.pending_settlements(), soroban_sdk::vec![&w.env, id]);
    assert_err(w.pool.try_deposit_lp(&lp, &(1_000 * USDC)), 113);
    assert_err(w.pool.try_deposit_senior(&lp, &(1_000 * USDC)), 113);
    w.pool.withdraw(&j, &(w.pool.lp_shares(&j) / 2)); // retraits ouverts
    w.set_spot("MXN", px(4, 2)); // règlement gagnant pour le pool
    w.pool.settle(&id);
    assert!(w.pool.pending_settlements().is_empty());
    w.pool.deposit_lp(&lp, &(1_000 * USDC));
    w.assert_state();
}

/// FUTUR B3 × B1 : une couverture sans cours ne bloque les dépôts que pendant l'heure de grâce.
#[cfg(any())]
#[test]
fn f_b3_hedge_without_fixing_blocks_deposits_only_during_grace() {
    let w = world_mxn();
    w.junior(100_000 * USDC);
    let lp = Address::generate(&w.env);
    w.pool.set_allowed(&Role::Lp, &lp, &true);
    w.mint(&lp, 10_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 100_000 * UNIT, 2 * 3_600, 1000);
    let m = w.pool.get_hedge(&id).unwrap().maturity;
    w.warp(m + 10);
    assert_err(w.pool.try_deposit_lp(&lp, &(1_000 * USDC)), 113);
    w.warp(m + 3_600 + 1);
    assert_err(w.pool.try_settle(&id), E_NO_FIXING);
    w.pool.deposit_lp(&lp, &(1_000 * USDC)); // fenêtre écoulée : accepté
    w.assert_state();
}

/// FUTUR B1 : `settle_fallback` clôt sans gain ni perte une couverture sans cours ; R retombe à 0
/// et le dernier LP peut tout retirer (complète `r_last_lp_with_hedge_without_fixing`).
#[test]
fn f_b1_settle_fallback_releases_reserve() {
    let w = world_mxn();
    let j = w.junior(10_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let before = w.balance(&sme);
    let id = w.open(&sme, "MXN", Side::BuyCurrency, 100_000 * UNIT, 2 * 3_600, 1000);
    let m = w.pool.get_hedge(&id).unwrap().maturity;
    w.warp(m + 3_600 - 1);
    assert_err(w.pool.try_settle_fallback(&id), 209); // heure non écoulée : un cours peut encore arriver
    w.warp(m + 3_600 + 1);
    w.pool.settle_fallback(&id); // B4 : crédite la marge au lieu de la transférer
    assert_eq!(w.pool.total_assets(), 10_000 * USDC); // ni gain ni perte
    assert_eq!(w.balance(&sme) + w.pool.claimable(&sme), before);
    w.pool.withdraw(&j, &w.pool.lp_shares(&j)); // R = 0 : sortie totale
    assert_eq!(w.balance(&w.pool.address), 500 * USDC, "reste la marge réclamable");
    w.claim_all();
    assert_eq!(w.balance(&w.pool.address), 0);
}

/// FUTUR B4 : `settle` crédite un solde réclamable (valeur de retour = montant crédité),
/// `close_early` garde le versement direct, `claim_for` (ouvert à tous) paie toujours le
/// propriétaire, 209 quand il n'y a rien. Invariant : solde = T + marges ouvertes + réclamables.
/// Avec B4, `World::claimable_total` devient Σ `pool.claimable(owner)` sur les propriétaires des
/// couvertures (+ part du protocole avec B11), et `settle_at` peut appeler `claim_for`.
#[test]
fn f_b4_claimable_balance_and_claim_for() {
    let w = world_mxn();
    w.junior(100_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let start = w.balance(&sme);
    let a = w.open(&sme, "MXN", Side::BuyCurrency, 100_000 * UNIT, 7 * DAY, 1000); // marge 500
    let b = w.open(&sme, "MXN", Side::BuyCurrency, 200_000 * UNIT, 14 * DAY, 1000); // marge 1 000
    let c = w.open(&sme, "MXN", Side::BuyCurrency, 100_000 * UNIT, 30 * DAY, 1000); // marge 500
    let after_margins = w.balance(&sme);
    assert_eq!(w.settle_at(a, "MXN", px(55, 3)), 1_000 * USDC); // +10 % : 500 + 500
    assert_eq!(w.balance(&sme), after_margins, "rien n'est transféré au règlement");
    assert_eq!(w.pool.claimable(&sme), 1_000 * USDC);
    assert_eq!(w.balance(&w.pool.address), w.pool.total_assets() + 1_500 * USDC + 1_000 * USDC);
    assert_eq!(w.settle_at(b, "MXN", px(52, 3)), 1_400 * USDC); // +4 % : 1 000 + 400, cumul
    assert_eq!(w.pool.claimable(&sme), 2_400 * USDC);
    w.advance(DAY);
    w.set_spot("MXN", px(5, 2));
    assert_eq!(w.pool.close_early(&c), 500 * USDC); // versement direct, à plat
    assert_eq!(w.balance(&sme), after_margins + 500 * USDC);
    // un tiers appelle claim_for : les fonds vont au propriétaire
    assert_eq!(w.pool.claim_for(&sme), 2_400 * USDC);
    assert_eq!(w.balance(&sme), start + 400 * USDC + 500 * USDC);
    assert_eq!(w.pool.claimable(&sme), 0);
    assert_eq!(w.pool.claim_for(&sme), 0, "rien à verser : 0, sans erreur");
    assert_eq!(w.pool.claim(&sme), 0);
    w.assert_state();
}

/// FUTUR B11 : la part du protocole n'est jamais prélevée sur un règlement perdant pour le pool,
/// même quand la perte égale tout le capital (T = perte, pas de 103).
/// Signatures `set_protocol_fee(bps, treasury)` / `protocol_claimable()` à confirmer.
#[cfg(any())]
#[test]
fn f_b11_no_fee_on_pool_loss_even_at_t_equals_loss() {
    let w = world_mxn();
    w.pool.set_spread(&50);
    w.pool.set_protocol_fee(&2_000, &Address::generate(&w.env)); // 20 % du spread
    w.junior(2_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    // acheteur : taux 0,05025, marge 1 005 ; vendeur : taux 0,04975, marge 995 ; capacité 0
    let buy = w.open(&sme, "MXN", Side::BuyCurrency, 200_000 * UNIT, 7 * DAY, 1000);
    let sell = w.open(&sme, "MXN", Side::SellCurrency, 200_000 * UNIT, 14 * DAY, 1000);
    assert_eq!(w.pool.capacity(), 0);
    w.settle_at(buy, "MXN", px(6, 2)); // pool perd 1 005 : T = 995 = marge du vendeur
    assert_eq!(w.pool.protocol_claimable(), 0);
    let r = w.pool.try_settle(&sell); // pas encore échue : rien
    assert!(r.is_err());
    w.settle_at(sell, "MXN", px(4, 2)); // pool perd 995 = T : doit passer
    assert_eq!(w.pool.total_assets(), 0);
    assert_eq!(w.pool.protocol_claimable(), 0);
    w.assert_state();
}

/// FUTUR B11 : sur un gain du pool, 0 ≤ part ≤ gain ; T monte de gain − part ; la part est
/// créditée au protocole ; règlement à plat : aucune part.
#[cfg(any())]
#[test]
fn f_b11_fee_bounded_by_pool_gain() {
    let w = world_mxn();
    w.pool.set_spread(&50);
    w.pool.set_protocol_fee(&2_000, &Address::generate(&w.env));
    w.junior(100_000 * USDC);
    let sme = w.sme(10_000 * USDC);
    let win = w.open(&sme, "MXN", Side::BuyCurrency, 200_000 * UNIT, 7 * DAY, 1000);
    let flat = w.open(&sme, "MXN", Side::BuyCurrency, 200_000 * UNIT, 14 * DAY, 1000);
    let t0 = w.pool.total_assets();
    let h = w.pool.get_hedge(&win).unwrap();
    w.settle_at(win, "MXN", px(4, 2)); // l'acheteur perd sa marge : gain du pool = 1 005
    let fee = w.pool.protocol_claimable();
    assert!(fee > 0 && fee <= h.margin, "part {fee}");
    assert_eq!(w.pool.total_assets(), t0 + h.margin - fee);
    let locked = w.pool.get_hedge(&flat).unwrap().locked_rate;
    w.settle_at(flat, "MXN", locked); // P&L nul
    assert_eq!(w.pool.protocol_claimable(), fee);
    w.assert_state();
}
