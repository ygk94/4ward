// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
//! Filet de tests du vault : `assert_state()` (invariants au repos) et suites d'opérations
//! pseudo-aléatoires passant par les vrais points d'entrée (`deposit_lp`, `withdraw`,
//! `deposit_senior`, `withdraw_senior`, `open_hedge`, `settle`).
//!
//! Rejouer un échec : le message donne `seed`, `suite`, `pas`, l'état et les dernières opérations,
//! et la commande exacte (`NET_SEED`, `NET_CHECKS`, `NET_SUITE`) qui rejoue cette suite seule en
//! imprimant chaque pas. `NET_SUITES` / `NET_STEPS` allongent une campagne (essai de nuit).
extern crate std;

use super::*;
use crate::{Pool, PoolClient, Role, Side, BPS};
use mock_oracle::{Asset as OracleAsset, MockOracle, MockOracleClient};
use soroban_sdk::testutils::{Address as _, EnvTestConfig, Ledger as _};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env, InvokeError, Symbol};
use std::format;
use std::string::String;
use std::vec::Vec;

const USDC: i128 = 10_000_000;
const HOUR: u64 = 3_600;
const DAY: u64 = 86_400;
const YEAR: u64 = 31_536_000;
/// 0,055 USD par MXN (14 décimales).
const SPOT_0: i128 = 5_500_000_000_000;
const SCALE: i128 = 100_000_000_000_000; // 10^PRICE_DECIMALS
/// Plancher de parts junior de B6. Après B6 : remplacer par `super::MIN_JUNIOR_SHARES`.
const FLOOR_SHARES: i128 = 10_000_000;

// ---------------------------------------------------------------------------------------------
// Environnement
// ---------------------------------------------------------------------------------------------

/// Invariants ROUGES aujourd'hui, activables un par un (masque de bits). Les invariants VERTS
/// sont toujours vérifiés.
pub(super) type Checks = u32;
/// I1 T ≥ R.
pub(super) const T_GE_R: Checks = 1 << 0;
/// I6 capacité ≥ 0.
pub(super) const CAP_POS: Checks = 1 << 1;
/// I10 Σ senior_balance ≤ T − R.
pub(super) const SENIOR_LE_FREE: Checks = 1 << 2;
/// WS un retrait senior ne verse jamais plus que T − R.
pub(super) const WS_LE_FREE: Checks = 1 << 3;
/// S1 aucun règlement refusé en 103 (sinon toléré : la couverture reste ouverte, « bloquée »).
pub(super) const NO_103: Checks = 1 << 4;
/// I11 R > 0 ⇒ parts junior ≥ plancher.
pub(super) const FLOOR: Checks = 1 << 5;
/// I12 aucune part junior ⇒ aucune valeur junior.
pub(super) const ORPHAN: Checks = 1 << 6;
/// D1 un dépôt junior n'est pas dilué au-delà d'une part d'arrondi.
pub(super) const DILUTION: Checks = 1 << 7;
/// RT dépôt puis retrait immédiat ne rend jamais plus que déposé.
pub(super) const ROUND_TRIP: Checks = 1 << 8;

/// Groupe corrigé par B5 (intérêts Safe plafonnés, retrait senior).
pub(super) const B5: Checks = T_GE_R | CAP_POS | SENIOR_LE_FREE | WS_LE_FREE | NO_103;
/// Groupe corrigé par B6 (plancher de parts junior).
pub(super) const B6: Checks = FLOOR | ORPHAN | DILUTION | ROUND_TRIP;
pub(super) const ALL: Checks = B5 | B6;
pub(super) const TODAY: Checks = 0;

/// Couverture ouverte, vue par le modèle fantôme.
#[derive(Clone, Debug)]
pub(super) struct Open {
    pub id: u64,
    pub side: Side,
    pub margin: i128,
    /// Notionnel en USDC au taux bloqué, même calcul que le vault (arrondi vers le bas).
    pub notional_usd: i128,
    pub locked: i128,
    pub maturity: u64,
    /// Cours d'échéance choisi à l'ouverture (fixé dans l'oracle par le « keeper »).
    pub fixing: i128,
    /// Règlement refusé en 103 alors que `NO_103` est désactivé : la couverture reste ouverte.
    pub stuck: bool,
}

/// État lu d'un coup (voir `Net::snapshot`).
struct Snap {
    t: i128,
    r: i128,
    tv: TrancheValues,
    cap: i128,
    e: Exposure,
    tn: i128,
    ep: i128,
    sp: i128,
    js: i128,
    sst: i128,
    /// Σ sur les LP : parts junior, lp_balance, parts senior, senior_balance.
    sh: i128,
    lb: i128,
    ssh: i128,
    sb: i128,
}

pub(super) struct Net {
    pub env: Env,
    pub pool: Address,
    pub usdc: Address,
    pub oracle: Address,
    pub lps: Vec<Address>,
    pub sme: Address,
    pub open: Vec<Open>,
    pub checks: Checks,
}

fn mxn(env: &Env) -> Symbol {
    Symbol::new(env, "MXN")
}

impl Net {
    pub fn new(checks: Checks, n_lps: usize) -> Net {
        // Pas d'instantané JSON à la destruction : des milliers d'environnements par campagne.
        let env = Env::new_with_config(EnvTestConfig { capture_snapshot_at_drop: false });
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();
        env.ledger().set_timestamp(1_000_000);
        let admin = Address::generate(&env);
        let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
        let oracle = env.register(MockOracle, (&admin,));
        let pool = env.register(Pool, (&admin, &usdc, &oracle, &50u32));
        PoolClient::new(&env, &pool).set_senior_deposits_open(&true); // fermés par défaut (B5)
        let c = PoolClient::new(&env, &pool);
        c.set_rate(&Symbol::new(&env, "USD"), &400);
        c.set_rate(&mxn(&env), &700);
        c.set_min_cap(&mxn(&env), &500);
        let mut lps = Vec::new();
        for _ in 0..n_lps {
            let lp = Address::generate(&env);
            c.set_allowed(&Role::Lp, &lp, &true);
            StellarAssetClient::new(&env, &usdc).mint(&lp, &(1_000_000_000 * USDC));
            lps.push(lp);
        }
        let sme = Address::generate(&env);
        StellarAssetClient::new(&env, &usdc).mint(&sme, &(1_000_000_000 * USDC));
        let net = Net { env, pool, usdc, oracle, lps, sme, open: Vec::new(), checks };
        net.set_spot(SPOT_0);
        net
    }

    fn on(&self, c: Checks) -> bool {
        self.checks & c != 0
    }

    pub fn client(&self) -> PoolClient<'_> {
        PoolClient::new(&self.env, &self.pool)
    }

    fn vault<R>(&self, f: impl FnOnce(&Env) -> R) -> R {
        self.env.as_contract(&self.pool, || f(&self.env))
    }

    pub fn now(&self) -> u64 {
        self.env.ledger().timestamp()
    }

    fn set_time(&self, t: u64) {
        self.env.ledger().set_timestamp(t);
    }

    fn set_spot(&self, price: i128) {
        MockOracleClient::new(&self.env, &self.oracle).set_price(&OracleAsset::Other(mxn(&self.env)), &price);
    }

    fn usdc_balance(&self, who: &Address) -> i128 {
        TokenClient::new(&self.env, &self.usdc).balance(who)
    }

    // ---- lectures internes ----

    pub fn t(&self) -> i128 {
        self.vault(total_deposits)
    }
    pub fn r(&self) -> i128 {
        self.vault(reserved)
    }
    /// Valeur junior recalculée par le test, sur la créance senior NON plafonnée :
    /// T − min(parts senior × prix couru, T). Indépendante de la formule du contrat, elle attrape
    /// un prix d'entrée junior calculé sur la valeur plafonnée (formule « naïve » de B5).
    pub fn true_junior(&self) -> i128 {
        self.vault(|env| {
            let t = total_deposits(env);
            let claim = mul_div(env, senior_total_shares(env), senior_price_now(env), SENIOR_PRICE_ONE);
            (t - claim.min(t.max(0))).max(0)
        })
    }

    pub fn junior_shares_total(&self) -> i128 {
        self.vault(total_shares)
    }

    // -----------------------------------------------------------------------------------------
    // assert_state : invariants au repos (hors règlement en cours)
    // -----------------------------------------------------------------------------------------

    /// Lit tout l'état dans une seule trame du contrat, par les fonctions internes qu'appellent
    /// les points d'entrée de lib.rs (un appel client par lecture coûterait bien plus cher).
    fn snapshot(&self) -> Snap {
        let lps = self.lps.clone();
        let cur = mxn(&self.env);
        self.vault(|env| {
            let mut s = Snap {
                t: total_deposits(env),
                r: reserved(env),
                tv: tranche_values(env),
                cap: capacity(env),
                e: exposure(env, &cur),
                tn: total_notional(env),
                ep: exit_price(env),
                sp: share_price(env),
                js: total_shares(env),
                sst: senior_total_shares(env),
                sh: 0,
                lb: 0,
                ssh: 0,
                sb: 0,
            };
            for lp in lps.iter() {
                s.sh += lp_shares(env, lp);
                s.lb += balance(env, lp);
                s.ssh += senior_shares(env, lp);
                s.sb += senior_balance(env, lp);
            }
            s
        })
    }

    pub fn state(&self) -> Result<(), String> {
        let Snap { t, r, tv, cap, e, tn, ep, sp, js, sst, sh, lb, ssh, sb } = self.snapshot();
        let open: Vec<&Open> = self.open.iter().collect();
        let sum_margin: i128 = open.iter().map(|o| o.margin).sum();
        let sum_notional: i128 = open.iter().map(|o| o.notional_usd).sum();

        // I1 R ≥ 0 (VERT) ; T ≥ R (ROUGE B5 : retrait senior sous plafond)
        if r < 0 {
            return Err(format!("I1 R < 0 : R={r}"));
        }
        if self.on(T_GE_R) && t < r {
            return Err(format!("I1 T < R : T={t} R={r}"));
        }
        // I2 solde USDC = T + Σ marges ouvertes (VERT ; + Σ réclamable après B4)
        let bal = self.usdc_balance(&self.pool);
        if bal != t + sum_margin {
            return Err(format!("I2 solde {bal} ≠ T {t} + marges {sum_margin}"));
        }
        // I3 R = Σ marges ouvertes = Σ marges des expositions (VERT, une seule devise ici)
        if r != sum_margin || e.buy_margin + e.sell_margin != r {
            return Err(format!("I3 R={r}, Σ marges fantôme={sum_margin}, exposition={}", e.buy_margin + e.sell_margin));
        }
        // I3c capital réservé par propriétaire = R (une seule PME ici) : attrape un futur chemin de
        // clôture qui ne passerait pas par close_position (VERT)
        let owner_r = self.env.as_contract(&self.pool, || super::owner_reserved(&self.env, &self.sme));
        if owner_r != r {
            return Err(format!("I3c réservé du propriétaire {owner_r} ≠ R {r}"));
        }
        let buy: i128 = open.iter().filter(|o| o.side == Side::BuyCurrency).map(|o| o.notional_usd).sum();
        if e.buy_notional != buy || e.buy_notional + e.sell_notional != sum_notional || e.net_notional != e.buy_notional - e.sell_notional {
            return Err(format!("I3b exposition {e:?} ≠ fantôme (achat {buy}, total {sum_notional})"));
        }
        // I4 TotalNotional = Σ notionnels ouverts (VERT)
        if tn != sum_notional {
            return Err(format!("I4 TotalNotional {tn} ≠ {sum_notional}"));
        }
        // I5 senior + junior = T, tous deux ≥ 0 ; capacité ≤ max(junior − R, 0) (VERT)
        if tv.senior + tv.junior != t || tv.senior < 0 || tv.junior < 0 {
            return Err(format!("I5 tranches {tv:?}, T={t}"));
        }
        // capacité ≤ max(junior − R, 0) : égalité aujourd'hui ; après B5/B6 la capacité est bornée
        // à 0, diminuée d'une provision d'intérêts, et nulle sous le plancher de parts.
        if cap > (tv.junior - r).max(0) {
            return Err(format!("I5 capacité {cap} incohérente avec junior {} − R {r}", tv.junior));
        }
        // I6 capacité ≥ 0 (ROUGE B5 : intérêts senior)
        if self.on(CAP_POS) && cap < 0 {
            return Err(format!("I6 capacité négative {cap} (junior {}, R {r})", tv.junior));
        }
        // I7 0 ≤ exit_price ≤ share_price (VERT)
        if ep < 0 || ep > sp {
            return Err(format!("I7 exit_price {ep} > share_price {sp}"));
        }
        // I8 Σ parts LP = TotalShares ; Σ lp_balance ≤ capital libre (VERT)
        let free = cap.max(0);
        if sh != js || lb > free {
            return Err(format!("I8 Σ parts {sh} / total {js} ; Σ lp_balance {lb} > libre {free}"));
        }
        // I9 Σ parts senior = total ; Σ senior_balance ≤ valeur senior (VERT)
        if ssh != sst || sb > tv.senior {
            return Err(format!("I9 Σ parts senior {ssh} / {sst} ; Σ senior_balance {sb} > {}", tv.senior));
        }
        // I10 Σ senior_balance ≤ T − R : le senior ne peut pas emporter le capital réservé (ROUGE B5)
        if self.on(SENIOR_LE_FREE) && sb > (t - r).max(0) {
            return Err(format!("I10 Σ senior_balance {sb} > T − R = {}", t - r));
        }
        // I11 R > 0 ⇒ parts junior ≥ plancher (ROUGE B6)
        if self.on(FLOOR) && r > 0 && js < FLOOR_SHARES {
            return Err(format!("I11 R={r} > 0 avec {js} parts junior < plancher {FLOOR_SHARES}"));
        }
        // I12 aucune part junior ⇒ aucune valeur junior (capital orphelin, capté par le déposant
        // suivant à 1:1) (ROUGE B6)
        if self.on(ORPHAN) && js == 0 && tv.junior > 0 {
            return Err(format!("I12 junior orphelin : {} stroops sans aucune part junior (R={r})", tv.junior));
        }
        Ok(())
    }

    #[allow(dead_code)] // pour les tests nommés de B5 / B6
    pub fn assert_state(&self) {
        if let Err(e) = self.state() {
            panic!("assert_state : {e}");
        }
    }

    // -----------------------------------------------------------------------------------------
    // Opérations (Ok = exécutée ou refus prévu ; Err = invariant d'opération violé ou code imprévu)
    // -----------------------------------------------------------------------------------------

    pub fn deposit(&self, lp: usize, amount: i128) -> Result<String, String> {
        let c = self.client();
        let lp_a = &self.lps[lp];
        let (j0, s0, r0) = (self.true_junior(), self.junior_shares_total(), self.r());
        let before = c.lp_shares(lp_a);
        match code(c.try_deposit_lp(lp_a, &amount)) {
            Ok(after) => {
                let minted = after - before;
                let (j1, s1) = (self.true_junior(), self.junior_shares_total());
                let value = mul(minted, j1, s1);
                // Perte d'arrondi tolérée : la valeur d'un stroop de part au prix d'avant, + 10 stroops.
                // Avec R > 0, le nombre de parts est borné par le plancher (B6) : une poussière de
                // parts adossée à du capital réservé (l'inflation) dépasse donc la tolérance. Avec
                // R = 0, un LP peut légitimement descendre sous le plancher sans changer le prix de
                // la part : seule compte la valeur d'une part, qui peut être élevée après des gains.
                let parts = if r0 > 0 { s0.max(FLOOR_SHARES) } else { s0.max(1) };
                let tol = ceil_div(j0, parts) + 10;
                if self.on(DILUTION) && amount - value > tol {
                    return Err(format!(
                        "D1 dépôt dilué : {amount} déposés → {minted} parts valant {value} (perte {} > {tol})",
                        amount - value
                    ));
                }
                Ok(format!("deposit lp{lp} {amount} → {minted} parts"))
            }
            Err(e) => allowed(e, &[104, 105], "deposit_lp"),
        }
    }

    pub fn withdraw(&self, lp: usize, shares: i128) -> Result<String, String> {
        let c = self.client();
        let cap = c.capacity();
        match code(c.try_withdraw(&self.lps[lp], &shares)) {
            Ok(paid) => {
                if paid > cap.max(0) {
                    return Err(format!("W1 retrait {paid} > capacité {cap}"));
                }
                Ok(format!("withdraw lp{lp} {shares} parts → {paid}"))
            }
            Err(e) => allowed(e, &[100, 107, 111, 116], "withdraw"),
        }
    }

    /// Dépôt puis retrait immédiat des parts reçues : jamais plus que déposé (ROUGE B6 : capital
    /// orphelin quand la dernière part junior est brûlée avec R > 0).
    pub fn round_trip(&self, lp: usize, amount: i128) -> Result<String, String> {
        let c = self.client();
        let before = c.lp_shares(&self.lps[lp]);
        match code(c.try_deposit_lp(&self.lps[lp], &amount)) {
            Ok(after) => match code(c.try_withdraw(&self.lps[lp], &(after - before))) {
                Ok(paid) if self.on(ROUND_TRIP) && paid > amount => Err(format!("RT aller-retour : {amount} déposés, {paid} rendus")),
                Ok(paid) => Ok(format!("round_trip lp{lp} {amount} → {paid}")),
                Err(e) => allowed(e, &[107, 111, 116], "withdraw (aller-retour)"),
            },
            Err(e) => allowed(e, &[104, 105], "deposit_lp (aller-retour)"),
        }
    }

    pub fn deposit_senior(&self, lp: usize, amount: i128) -> Result<String, String> {
        let c = self.client();
        let lp_a = &self.lps[lp];
        let before = c.senior_balance(lp_a);
        match code(c.try_deposit_senior(lp_a, &amount)) {
            Ok(_) => {
                let gained = c.senior_balance(lp_a) - before;
                // prix ≈ 1 stroop par part : 2 stroops d'arrondi au plus
                if amount - gained > 2 {
                    return Err(format!("DS dépôt senior dilué : {amount} déposés, valeur +{gained}"));
                }
                Ok(format!("deposit_senior lp{lp} {amount}"))
            }
            // 118 intérêts en retard, 119 dépôts Safe fermés : après B5
            Err(e) => allowed(e, &[105, 111, 118, 119], "deposit_senior"),
        }
    }

    pub fn withdraw_senior(&self, lp: usize, shares: i128) -> Result<String, String> {
        let c = self.client();
        let room = self.t() - self.r();
        match code(c.try_withdraw_senior(&self.lps[lp], &shares)) {
            Ok(paid) => {
                if self.on(WS_LE_FREE) && paid > room.max(0) {
                    return Err(format!("WS retrait senior {paid} > T − R = {room} (T devient {} < R {})", self.t(), self.r()));
                }
                Ok(format!("withdraw_senior lp{lp} {shares} parts → {paid}"))
            }
            Err(e) => allowed(e, &[100, 107, 117], "withdraw_senior"),
        }
    }

    /// Ouvre une couverture MXN dont la marge vaut ≈ `target_margin`, et choisit dès maintenant
    /// son cours d'échéance : `outcome` 0 = le pool perd la marge, 1 = le pool gagne la marge,
    /// 2 = neutre (cours = taux bloqué), 3+ = cours dans ±1,5 plafond (`frac` en ‰ de −1000 à 1000).
    pub fn open_hedge(&mut self, side: Side, target_margin: i128, duration: u64, cap_bps: u32, outcome: u64, frac: i128) -> Result<String, String> {
        if target_margin <= 0 {
            return Ok(String::from("open : marge cible nulle, ignorée"));
        }
        self.set_spot(SPOT_0); // prix frais (≤ 600 s) exigé à l'ouverture
        // notionnel ≈ marge × BPS / plafond / spot ; le taux bloqué diffère un peu du spot (101 possible)
        let notional = target_margin * BPS / cap_bps as i128 * SCALE / SPOT_0;
        if notional <= 0 {
            return Ok(String::from("open : notionnel nul, ignorée"));
        }
        let c = self.client();
        let r = code(c.try_open_hedge(&self.sme, &self.sme, &mxn(&self.env), &side, &notional, &duration, &cap_bps));
        match r {
            Ok(id) => {
                let h = c.get_hedge(&id).unwrap();
                let move_ = h.locked_rate * cap_bps as i128 / BPS; // écart qui épuise la marge
                let user_up = |k: i128| match side {
                    Side::BuyCurrency => h.locked_rate + k,
                    Side::SellCurrency => h.locked_rate - k,
                };
                let fixing = match outcome {
                    0 => user_up(2 * move_),
                    1 => user_up(-2 * move_),
                    2 => h.locked_rate,
                    _ => user_up(move_ * 3 * frac / 2000),
                }
                .max(1);
                let notional_usd = h.notional * h.locked_rate / SCALE;
                self.open.push(Open {
                    id,
                    side,
                    margin: h.margin,
                    notional_usd,
                    locked: h.locked_rate,
                    maturity: h.maturity,
                    fixing,
                    stuck: false,
                });
                Ok(format!("open #{id} {side:?} marge {} échéance +{}s issue {outcome}", h.margin, duration))
            }
            Err(e) => allowed(e, &[101], "open_hedge"),
        }
    }

    /// Avance le temps de `dt` ; le keeper règle chaque couverture à son échéance, dans l'ordre.
    pub fn advance(&mut self, dt: u64) -> Result<String, String> {
        let target = self.now() + dt;
        let mut log = format!("advance {dt}s");
        loop {
            let next = self
                .open
                .iter()
                .enumerate()
                .filter(|(_, o)| !o.stuck && o.maturity <= target)
                .min_by_key(|(_, o)| (o.maturity, o.id))
                .map(|(i, _)| i);
            let Some(i) = next else { break };
            let o = self.open[i].clone();
            if o.maturity > self.now() {
                self.set_time(o.maturity);
            }
            self.set_spot(o.fixing);
            let t_before = self.t();
            match code(self.client().try_settle(&o.id)) {
                Ok(paid) => {
                    self.open.remove(i);
                    log += &format!(" | settle #{} payé {paid} (T {t_before} → {})", o.id, self.t());
                }
                Err(103) if !self.on(NO_103) => {
                    self.open[i].stuck = true;
                    log += &format!(" | settle #{} bloqué 103 (toléré)", o.id);
                }
                Err(c) => {
                    return Err(format!(
                        "S1 settle #{} refusé ({c}) : marge {}, cours {} / bloqué {}, T={} R={}",
                        o.id,
                        o.margin,
                        o.fixing,
                        o.locked,
                        self.t(),
                        self.r()
                    ))
                }
            }
            // invariants entre deux règlements aussi (état au repos)
            self.state()?;
        }
        self.set_time(target);
        Ok(log)
    }
}

// ---- utilitaires ----

type TryResult<T, E> = Result<Result<T, E>, Result<soroban_sdk::Error, InvokeError>>;

/// Code d'erreur de contrat d'un appel `try_*`, ou la valeur.
fn code<T, E: core::fmt::Debug>(r: TryResult<T, E>) -> Result<T, u32> {
    match r {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => panic!("conversion de la valeur de retour : {e:?}"),
        Err(Ok(e)) => {
            if e.is_type(soroban_sdk::xdr::ScErrorType::Contract) {
                Err(e.get_code())
            } else {
                panic!("erreur hôte inattendue : {e:?}")
            }
        }
        Err(Err(InvokeError::Contract(c))) => Err(c),
        Err(Err(e)) => panic!("abandon inattendu : {e:?}"),
    }
}

fn allowed(c: u32, codes: &[u32], op: &str) -> Result<String, String> {
    if codes.contains(&c) {
        Ok(format!("{op} refusé ({c})"))
    } else {
        Err(format!("{op} : code imprévu {c}"))
    }
}

fn mul(a: i128, b: i128, c: i128) -> i128 {
    if c == 0 {
        0
    } else {
        a * b / c
    }
}

fn ceil_div(a: i128, b: i128) -> i128 {
    (a + b - 1) / b
}

// ---------------------------------------------------------------------------------------------
// Générateur (LCG de pseudo_random_baskets_reserve_covers_grid) et moteur de suites
// ---------------------------------------------------------------------------------------------

pub(super) struct Lcg(pub u64);

impl Lcg {
    pub fn next(&mut self, modulo: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % modulo
    }
    /// Montant log-uniforme de 1 stroop à 10^max_pow stroops.
    fn amount(&mut self, max_pow: u32) -> i128 {
        let p = self.next(max_pow as u64 + 1) as u32;
        let base = 10i128.pow(p);
        base + (self.next(9) as i128) * base
    }
}

/// Réglages d'une campagne.
#[derive(Clone, Copy)]
pub(super) struct Config {
    /// Nom du test, pour la commande de rejeu.
    pub name: &'static str,
    pub seed: u64,
    pub suites: u32,
    pub steps: u32,
    pub checks: Checks,
    /// Opérations senior (dépôt / retrait) autorisées.
    pub senior: bool,
    /// Opérations ciblées : « tout sauf 1 stroop de part », « saturer la capacité », « un an ».
    pub targeted: bool,
}

/// Exécute une suite ; Err(message complet de rejeu) au premier échec.
pub(super) fn run_suite(cfg: &Config, suite: u32, verbose: bool) -> Result<(), String> {
    let suite_seed = cfg.seed ^ (suite as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut g = Lcg(suite_seed);
    let mut net = Net::new(cfg.checks, 4);
    let mut history: Vec<String> = Vec::new();
    // Départ : un junior, et la moitié du temps un senior à 4 × le junior (ratio pile).
    let j0 = (50_000 + g.next(450_000) as i128) * USDC;
    let mut start = vec_push(net.deposit(0, j0));
    if cfg.senior && g.next(2) == 0 {
        start += &vec_push(net.deposit_senior(1, 4 * j0));
    }
    history.push(start);

    for step in 0..cfg.steps {
        let lp = g.next(net.lps.len() as u64) as usize;
        let n_ops = if cfg.targeted { 12 } else { 9 };
        let op = g.next(n_ops);
        let res: Result<String, String> = match op {
            0 => net.deposit(lp, g.amount(13)),
            1 => {
                let owned = net.client().lp_shares(&net.lps[lp]);
                if owned == 0 {
                    history.push(format!("pas {step} : withdraw lp{lp} sans parts, ignoré"));
                    continue;
                }
                let sh = match g.next(4) {
                    0 => owned,
                    1 => owned / 2,
                    2 => owned * (1 + g.next(1000) as i128) / 1000,
                    _ => g.amount(12).min(owned),
                };
                net.withdraw(lp, sh)
            }
            2 if cfg.senior => net.deposit_senior(lp, g.amount(13)),
            3 if cfg.senior => {
                let owned = net.vault(|env| senior_shares(env, &net.lps[lp]));
                if owned == 0 {
                    history.push(format!("pas {step} : withdraw_senior lp{lp} sans parts, ignoré"));
                    continue;
                }
                let sh = match g.next(3) {
                    0 => owned,
                    1 => owned / 2,
                    _ => owned * (1 + g.next(1000) as i128) / 1000,
                };
                net.withdraw_senior(lp, sh)
            }
            4 | 5 => {
                let cap = net.client().capacity().max(0);
                let side = if g.next(2) == 0 { Side::BuyCurrency } else { Side::SellCurrency };
                let target = cap * (1 + g.next(100) as i128) / 100;
                let duration = HOUR + g.next(YEAR - HOUR);
                let cap_bps = [500u32, 1000, 1500, 2000][g.next(4) as usize];
                let (outcome, frac) = (g.next(5), g.next(2001) as i128 - 1000);
                net.open_hedge(side, target, duration, cap_bps, outcome, frac)
            }
            6 | 7 => {
                let dt = match g.next(3) {
                    0 => HOUR + g.next(DAY),
                    1 => DAY + g.next(30 * DAY),
                    _ => 30 * DAY + g.next(YEAR - 30 * DAY),
                };
                net.advance(dt)
            }
            8 => net.round_trip(lp, g.amount(12)),
            // ---- ciblées ----
            9 => {
                // retirer tout sauf 1 stroop de part (inflation par la décote de sortie)
                let owned = net.client().lp_shares(&net.lps[lp]);
                if owned > 1 {
                    net.withdraw(lp, owned - 1)
                } else {
                    Ok(String::from("dust : rien à retirer"))
                }
            }
            10 => {
                // saturer la capacité avec une couverture longue, issue perdante pour le pool
                let cap = net.client().capacity().max(0);
                let side = if g.next(2) == 0 { Side::BuyCurrency } else { Side::SellCurrency };
                net.open_hedge(side, cap * 98 / 100, YEAR, 1000, 0, 0)
            }
            11 => net.advance(YEAR / 2 + g.next(YEAR / 2)),
            _ => Ok(String::from("noop (senior désactivé)")),
        };
        let res = res.and_then(|log| net.state().map(|_| log));
        match res {
            Ok(log) => {
                if verbose {
                    std::println!("[suite {suite} pas {step}] {log}");
                }
                history.push(format!("pas {step} : {log}"));
            }
            Err(e) => {
                let tail: Vec<&String> = history.iter().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect();
                let mut msg = format!(
                    "ÉCHEC seed={:#x} suite={suite} pas={step} op={op} : {e}\n  état : T={} R={} parts junior={} capacité={} tranches={:?} t={}\n  rejouer : NET_SEED={:#x} NET_CHECKS={:x} NET_SUITE={suite} cargo test -p pool {} -- --include-ignored --exact --nocapture\n  dernières opérations :",
                    cfg.seed,
                    net.t(),
                    net.r(),
                    net.junior_shares_total(),
                    net.client().capacity(),
                    net.client().tranche_values(),
                    net.now(),
                    cfg.seed,
                    cfg.checks,
                    cfg.name,
                );
                for h in tail {
                    msg += &format!("\n    {h}");
                }
                return Err(msg);
            }
        }
    }
    Ok(())
}

fn vec_push(r: Result<String, String>) -> String {
    r.unwrap_or_else(|e| panic!("départ : {e}"))
}

/// Lance une campagne ; `NET_SEED` / `NET_SUITE` pour rejouer une seule suite en verbeux.
pub(super) fn campaign(mut cfg: Config) -> Result<(), String> {
    if let Ok(s) = std::env::var("NET_SEED") {
        cfg.seed = u64::from_str_radix(s.trim_start_matches("0x"), 16).expect("NET_SEED hexadécimal");
    }
    if let Ok(m) = std::env::var("NET_CHECKS") {
        cfg.checks = u32::from_str_radix(m.trim_start_matches("0x"), 16).expect("NET_CHECKS hexadécimal");
    }
    if let Ok(n) = std::env::var("NET_SUITES") {
        cfg.suites = n.parse().expect("NET_SUITES entier");
    }
    if let Ok(n) = std::env::var("NET_STEPS") {
        cfg.steps = n.parse().expect("NET_STEPS entier");
    }
    if let Ok(n) = std::env::var("NET_SUITE") {
        let n: u32 = n.parse().expect("NET_SUITE entier");
        return run_suite(&cfg, n, true);
    }
    for suite in 0..cfg.suites {
        run_suite(&cfg, suite, false)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

const SEED: u64 = 0x5EED_2026;

/// VERT : toutes les opérations, invariants qui tiennent aujourd'hui.
#[test]
fn net_random_sequences_today_invariants() {
    let cfg = Config { name: "vault::net_props::net_random_sequences_today_invariants", seed: SEED, suites: 100, steps: 40, checks: TODAY, senior: true, targeted: false };
    if let Err(e) = campaign(cfg) {
        panic!("{e}");
    }
}

/// ROUGE (B6) : suites junior seulement, opérations ciblées, invariants B6.
#[test]
fn net_random_sequences_b6() {
    let cfg = Config { name: "vault::net_props::net_random_sequences_b6", seed: SEED, suites: 100, steps: 40, checks: B6, senior: false, targeted: true };
    if let Err(e) = campaign(cfg) {
        panic!("{e}");
    }
}

/// ROUGE (B5) : seniors et temps long, invariants B5.
#[test]
fn net_random_sequences_b5() {
    let cfg = Config { name: "vault::net_props::net_random_sequences_b5", seed: SEED, suites: 100, steps: 40, checks: B5, senior: true, targeted: true };
    if let Err(e) = campaign(cfg) {
        panic!("{e}");
    }
}

/// ROUGE (B5 + B6) : la campagne complète, cible finale.
#[test]
fn net_random_sequences_all() {
    let cfg = Config { name: "vault::net_props::net_random_sequences_all", seed: SEED, suites: 100, steps: 40, checks: ALL, senior: true, targeted: true };
    if let Err(e) = campaign(cfg) {
        panic!("{e}");
    }
}

// ---- scénarios ciblés (mêmes aides, chiffres du plan) ----

/// ROUGE (B6), défaut 1 : inflation par la décote de sortie. Aujourd'hui : I11 juste après le
/// retrait « tout sauf 1 stroop » ; sans I11 (masque `DILUTION`), D1 sur le dépôt de 999 USDC.
/// Après B6 : le retrait est refusé en 116 (code accepté par `withdraw`) et tout passe.
#[test]
fn net_defect1_inflation_by_exit_discount() {
    defect1(ALL);
}

pub(super) fn defect1(checks: Checks) {
    let mut n = Net::new(checks, 2);
    n.deposit(0, 1_000 * USDC).unwrap();
    // marge ≈ 500 USDC, 30 jours, issue neutre (cours = taux bloqué)
    n.open_hedge(Side::BuyCurrency, 500 * USDC, 30 * DAY, 1000, 2, 0).unwrap();
    n.state().unwrap();
    let owned = n.client().lp_shares(&n.lps[0]);
    n.withdraw(0, owned - 1).unwrap();
    n.state().unwrap();
    n.deposit(1, 999 * USDC).unwrap();
    n.state().unwrap();
    n.advance(30 * DAY).unwrap();
    n.state().unwrap();
    // couverture réglée sans P&L, R = 0 : la victime vaut ce qu'elle a déposé, à 1 USDC près
    let v = n.client().lp_balance(&n.lps[1]);
    assert!(v >= 998 * USDC, "victime : 999 USDC déposés, vaut {v}");
}

/// ROUGE (B5), défaut 2 : pool 200 000 junior / 800 000 senior, marge ≈ 190 000 sur un an,
/// le senior retire tout une heure avant l'échéance perdante. Aujourd'hui : WS (retrait > T − R) ;
/// sans WS (masque `NO_103`), le règlement échoue en 103. Après B5 : retrait limité à T − R,
/// règlement accepté, T = R puis T ≥ 0.
#[test]
fn net_defect2_senior_withdraw_then_losing_settle() {
    defect2(ALL);
}

pub(super) fn defect2(checks: Checks) {
    let mut n = Net::new(checks, 2);
    n.deposit(0, 200_000 * USDC).unwrap();
    n.deposit_senior(1, 800_000 * USDC).unwrap();
    // issue 0 : le pool perd toute la marge
    n.open_hedge(Side::BuyCurrency, 190_000 * USDC, YEAR, 1000, 0, 0).unwrap();
    n.advance(YEAR - HOUR).unwrap();
    let sh = n.vault(|env| senior_shares(env, &n.lps[1]));
    n.withdraw_senior(1, sh).unwrap();
    n.advance(HOUR).unwrap();
    n.state().unwrap();
    assert!(n.open.is_empty(), "couverture restée ouverte (règlement bloqué)");
}
