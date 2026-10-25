"""Portage exact, en entiers Python, des formules du contrat pool.

Sources : contracts/pool/src/pricing.rs (forward_rate, to_usdc, margin_for), hedge.rs (pnl),
vault.rs (SENIOR_RATE_BPS, MIN_JUNIOR_BPS). Mêmes arrondis que le contrat, toujours côté pool :
F sans spread tronqué, spread arrondi vers le haut (BuyCurrency) ou vers le bas (SellCurrency),
marge arrondie vers le haut, P&L arrondi vers le bas puis borné à ±marge.

Unités : prix en USD par unité de devise, 14 décimales ; montants (devise et USDC) en 7 décimales.
Lancé seul (`python sim/contract.py`), vérifie les valeurs de référence et les affiche.
"""

BPS = 10_000
SECS_PER_YEAR = 31_536_000
PRICE_DECIMALS = 14
PRICE_ONE = 10**PRICE_DECIMALS
AMOUNT_ONE = 10**7

SENIOR_RATE_BPS = 450   # vault.rs : taux fixe du senior
MIN_JUNIOR_BPS = 2_000  # vault.rs : junior ≥ 20 % du capital tant qu'il y a du senior

BUY, SELL = "BuyCurrency", "SellCurrency"

# Scénario de référence (vidéo, docs/points-a-trancher.md, docs/payment-cli.md).
REF = dict(
    notional=1_000_000 * AMOUNT_ONE,   # 1 000 000 MXN
    spot=5_500_000_000_000,            # 0,055 USD / MXN
    r_usd=400, r_dev=700,              # USD 4 %, MXN 7 % (taux en dur du setup)
    spread=50,                         # 0,5 %
    duration=90 * 86_400,              # 90 jours
    cap=1_000,                         # plafond 10 %
    crash=1_200,                       # +12 %
)
REFLECTOR_SPOT = 5_325_000_000_000     # cours Reflector du 25/10 (0,05325)

# Jeux de taux annuels en bps. « en-dur » : scripts/setup-testnet.sh (Fed, Banxico, Selic, proxy BADLAR, BCE) ;
# « etherfuse » : rendements des stablebonds poussés par scripts/sync-rates.sh (25/10). ARS absent d'Etherfuse.
RATE_SETS = {
    "en-dur": {"USD": 400, "MXN": 700, "BRL": 1_050, "ARS": 2_200, "EUR": 200},
    "etherfuse": {"USD": 360, "MXN": 555, "BRL": 1_148, "ARS": 2_200, "EUR": 210},
}
RATES = "en-dur"   # jeu actif, changé par use_rates()


def use_rates(name):
    """Choisit le jeu de taux de toute la simulation (REF["r_usd"], REF["r_dev"] pour le MXN)."""
    global RATES
    rates = RATE_SETS[name]
    RATES = name
    REF.update(r_usd=rates["USD"], r_dev=rates["MXN"])


def rates_label(name=None):
    """« taux en dur : USD 4 % / MXN 7 % »."""
    name = name or RATES
    r = RATE_SETS[name]
    pc = lambda b: f"{b / 100:g} %".replace(".", ",")  # noqa: E731
    return f"taux {'en dur' if name == 'en-dur' else 'Etherfuse'} : USD {pc(r['USD'])} / MXN {pc(r['MXN'])}"


def mul_div_floor(a, b, c):
    return a * b // c


def mul_div_ceil(a, b, c):
    return (a * b + c - 1) // c


def forward_rate(spot, r_usd, r_dev, spread, side, duration):
    """pricing::forward_rate : F = S × (BPS·an + r_usd·d) / (BPS·an + r_dev·d), puis le spread."""
    assert 0 < duration <= SECS_PER_YEAR
    year = BPS * SECS_PER_YEAR
    f = mul_div_floor(spot, year + r_usd * duration, year + r_dev * duration)
    if side == BUY:
        return mul_div_ceil(f, BPS + spread, BPS)
    return mul_div_floor(f, BPS - spread, BPS)


def to_usdc(notional, rate):
    """pricing::to_usdc : notionnel (7 déc.) × prix (14 déc.) → USDC (7 déc.)."""
    return notional * rate // PRICE_ONE


def margin_for(usdc_cost, cap):
    """pricing::margin_for : usdc_cost × cap / BPS, arrondi vers le haut."""
    return mul_div_ceil(usdc_cost, cap, BPS)


def pnl(side, notional, locked, spot, margin):
    """hedge::pnl : > 0 le pool doit à la PME. Arrondi vers le bas (div_euclid), borné à ±marge."""
    diff = spot - locked if side == BUY else locked - spot
    raw = (notional * diff) // PRICE_ONE
    return max(-margin, min(margin, raw))


def quote(notional, spot, r_usd, r_dev, spread, duration, cap, side=BUY):
    """pricing::quote (sans oracle ni contrôle de plafond)."""
    f = forward_rate(spot, r_usd, r_dev, spread, side, duration)
    cost = to_usdc(notional, f)
    return dict(spot=spot, forward_rate=f, usdc_cost=cost, margin=margin_for(cost, cap))


def bump(price, bps):
    """Même calcul que scripts/crash.sh : prix × (BPS + bps) / BPS, tronqué."""
    return price * (BPS + bps) // BPS


def settle_buy(notional, q, spot_end):
    """Importateur (BuyCurrency) : ce qu'il paie au total pour sa facture, avec et sans protection.

    Le path payment achète la devise au cours de l'échéance (DEX supposé aligné sur l'oracle) ;
    la protection versée par le pool (ou prise sur la marge) vient en déduction.
    """
    p = pnl(BUY, notional, q["forward_rate"], spot_end, q["margin"])
    unprotected = to_usdc(notional, spot_end)
    return dict(
        pnl=p,
        payout=q["margin"] + p,             # crédité au règlement
        unprotected=unprotected,
        protected=unprotected - p,
    )


def scenario(spot=None, r_usd=None, r_dev=None, crash=None, cap=None, notional=None, spread=None, duration=None):
    """Scénario de la démo : devis à l'ouverture, puis crash de `crash` bps sur le spot avant l'échéance.
    Paramètres absents : ceux de REF (jeu de taux actif compris)."""
    spot, r_usd, r_dev, crash, cap, notional, spread, duration = (
        REF[k] if v is None else v for k, v in zip(
            ("spot", "r_usd", "r_dev", "crash", "cap", "notional", "spread", "duration"),
            (spot, r_usd, r_dev, crash, cap, notional, spread, duration)))
    q = quote(notional, spot, r_usd, r_dev, spread, duration, cap)
    end = bump(spot, crash)
    return dict(quote=q, spot_end=end, **settle_buy(notional, q, end))


def usdc(x):
    """Montant 7 décimales → float en USDC (affichage seulement)."""
    return x / AMOUNT_ONE


def fr(x, dec=2):
    """12345.678 → « 12 345,68 »."""
    s = f"{x:,.{dec}f}"
    return s.replace(",", " ").replace(".", ",")


def self_test():
    """Valeurs de référence du contrat (tests Rust, testnet au centime), toujours avec les taux en dur.
    Lève AssertionError si un chiffre diffère."""
    hard = RATE_SETS["en-dur"]
    s = scenario(r_usd=hard["USD"], r_dev=hard["MXN"])
    q = s["quote"]
    checks = [
        ("forward_rate", q["forward_rate"], 5_487_305_413_412),
        ("usdc_cost", q["usdc_cost"], 548_730_541_341),
        ("margin", q["margin"], 54_873_054_135),
        ("spot après +12 %", s["spot_end"], 6_160_000_000_000),
        ("sans protection", s["unprotected"], 616_000_000_000),        # 61 600,00
        ("avec protection", round(usdc(s["protected"]), 2), 56_112.69),
    ]
    bad = [(name, got, want) for name, got, want in checks if got != want]
    if bad:
        raise AssertionError("portage du contrat faux : " + "; ".join(f"{n} = {g}, attendu {w}" for n, g, w in bad))
    return checks


if __name__ == "__main__":
    for name, got, _ in self_test():
        print(f"ok  {name:18s} {got}")
