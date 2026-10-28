# Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
"""Rendement des tranches senior / junior d'un pool de démarrage → out/tranches.md.

1. Tableau du README (« Les LPs : tranche senior et tranche junior ») : pool de 500 k$ (400 k$ senior à 4,5 %,
   100 k$ junior = 20 %, minimum du contrat), contrats de 10 k$ à 90 jours dont le sens est tiré au hasard.
   Mêmes graines et même ordre de tirages que la simulation d'origine : les chiffres sont comparés au README.
2. Sensibilité au netting et à Blend (ex-tranches2.py / netting.py) : pool de 1 M$ (800 k$ / 200 k$),
   volume compensé imposé, avec ou sans 20 % du capital sur Blend.

Monte-Carlo sur un an (4 échéances de 90 jours), 40 000 trajectoires. Peso : vol 13 %/an (hypothèse du README,
au-dessus de la vol mesurée dans calibration.md), sauts de ±15 % (probabilité 0,75 % par trimestre).
"""
import re
from pathlib import Path

import numpy as np

import contract as c
import style

ROOT = Path(__file__).resolve().parent.parent

SENIOR_RATE = c.SENIOR_RATE_BPS / c.BPS     # 4,5 %
MIN_JUNIOR = c.MIN_JUNIOR_BPS / c.BPS       # 20 %
CAP = c.REF["cap"] / c.BPS                  # plafond 10 %

PATHS = 40_000
QUARTERS = 4
VOL_Q = 0.13 * 0.5            # vol 13 %/an sur 90 jours (√0,25 = 0,5)
JUMP_P_Q = 0.0075             # probabilité d'un saut par trimestre
JUMP = 0.15
SPREAD_Q = 0.004 * 0.8        # 0,4 % par contrat de 90 j, dont 20 % pour le protocole
TBILL = 0.04                  # bons du Trésor tokenisés
CONV = 0.003                  # coût de conversion de la couverture en CETES, par an
HEDGE_SHARE = 0.7             # part du senior qui peut servir de couverture en CETES
JUNIOR_UTIL = 0.15            # utilisation maximale du junior


def quarter_moves(rng, n):
    """Mouvement du peso sur 90 jours : normale + sauts. Ordre des tirages identique aux scripts d'origine."""
    moves = rng.standard_normal(n) * VOL_Q
    jumps = (rng.random(n) < JUMP_P_Q) * np.where(rng.random(n) < 0.5, -JUMP, JUMP)
    return moves + jumps, jumps


def tranche_split(total, senior, junior):
    """Cascade : le senior touche son taux en premier, le junior absorbe le reste (perte bornée à son capital)."""
    sen = np.minimum(senior * SENIOR_RATE, total + junior)
    jun = np.maximum(total - sen, -junior)
    return sen, jun


# ---- 1. Pool de démarrage (tableau du README) ----

def small_pool(rng, n, p, senior=400_000, junior=100_000, size=10_000, cash_share=0.08):
    tvl = senior + junior
    u_cap = JUNIOR_UTIL * junior / CAP          # notionnel non couvert que le junior peut porter
    h_cap = HEDGE_SHARE * senior                # notionnel couvrable en CETES
    tot = np.zeros(PATHS)
    paired_share = []
    for _ in range(QUARTERS):
        importers = rng.binomial(n, p, PATHS)
        gross = n * size
        net = np.abs(2 * importers - n) * size
        paired = gross - net
        unhedged = np.minimum(net, u_cap)
        hedged = np.minimum(net - unhedged, h_cap)
        accepted = paired + unhedged + hedged
        paired_share.append((paired / gross).mean())
        r, jumps = quarter_moves(rng, PATHS)
        invested = tvl + CAP * accepted - cash_share * tvl      # dépôts + marges clients, hors réserve d'USDC
        tot += (invested * TBILL / 4 - hedged * CONV / 4 + accepted * SPREAD_Q
                - unhedged * np.clip(r, -CAP, CAP)
                + np.where(jumps != 0, -hedged * (JUMP - CAP) * 0.5, 0))
    _, jun = tranche_split(tot, senior, junior)
    return dict(paired=np.mean(paired_share), pool=tot.mean() / tvl, junior=jun.mean() / junior,
                loss=(jun < 0).mean(), p5=np.percentile(jun, 5) / junior)


def readme_rows():
    """{(contrats, % importateurs): [compensé, pool, junior, années perdantes, pire]} lus dans le README."""
    rows = {}
    text = (ROOT / "README.md").read_text(encoding="utf-8")
    for line in text.splitlines():
        m = re.match(r"\|\s*\**(\d+)\**\s*\|\s*\**(\d+) k\$\**\s*\|\s*(\d+) %\s*\|(.*)", line)
        if not m:
            continue
        cells = [re.sub(r"[*\s%]", "", x).replace("−", "-").replace(",", ".") for x in m.group(4).split("|")[:5]]
        rows[(int(m.group(1)), int(m.group(3)))] = [float(x) for x in cells]
    return rows


# ---- 2. Sensibilité netting / Blend (ex-tranches2.py) ----

def netting_pool(rng, netted_q, blend_cap, blend=0.065, senior=800_000, junior=200_000, cash_share=0.08):
    tvl = senior + junior
    unhedged = JUNIOR_UTIL * junior / CAP
    hedged = HEDGE_SHARE * senior
    margins = CAP * (unhedged + hedged + netted_q)
    liquid = tvl + margins - hedged
    cash = cash_share * tvl
    on_blend = min(blend_cap * tvl, liquid - cash)
    tbill = liquid - cash - on_blend
    carry = on_blend * blend + tbill * TBILL + hedged * TBILL - hedged * CONV   # CETES : rendement = r_USD du pricing
    tot = np.full(PATHS, carry)
    for _ in range(QUARTERS):
        r, jumps = quarter_moves(rng, PATHS)
        tot += (-unhedged * np.clip(r, -CAP, CAP) + (unhedged + hedged + netted_q) * SPREAD_Q
                + np.where(jumps != 0, -hedged * (JUMP - CAP) * 0.5, 0))
    _, jun = tranche_split(tot, senior, junior)
    share = netted_q / (netted_q + unhedged + hedged)
    return dict(share=share, pool=tot.mean() / tvl, junior=jun.mean() / junior, loss=(jun < 0).mean(),
                p5=np.percentile(jun, 5) / junior)


def pct(x, dec=1):
    return f"{x * 100:.{dec}f} %".replace(".", ",").replace("-", "−")


def main():
    assert 100_000 / 500_000 >= MIN_JUNIOR and 200_000 / 1_000_000 >= MIN_JUNIOR

    rng = np.random.default_rng(3)
    results = {}
    for n in (20, 40, 80):
        for p in (0.5, 0.6, 0.7):
            results[(n, round(p * 100))] = small_pool(rng, n, p)

    readme = readme_rows()
    mismatches = []
    lines = [
        "# Tranches senior / junior",
        "",
        "Généré par `sim/tranches.py`. Ne pas éditer à la main.",
        "",
        f"Constantes du contrat : senior {pct(SENIOR_RATE)}, junior ≥ {pct(MIN_JUNIOR, 0)} du capital, plafond "
        f"{pct(CAP, 0)}. Hypothèses du modèle : peso 13 %/an + sauts de 15 %, spread 0,4 % par contrat de 90 j dont "
        "20 % pour le protocole, 8 % en USDC, le reste en bons du Trésor à 4 %, pas de Blend.",
        "",
        "## 1. Pool de démarrage (tableau du README)",
        "",
        "500 k$ (400 k$ senior, 100 k$ junior utilisé à 15 % max), contrats de 10 k$ à 90 jours.",
        "",
        "| Contrats / trimestre | Volume / trimestre | Part d'importateurs | Volume compensé | Pool | Junior (moyenne) "
        "| Années perdantes | Pire année sur 20 | README |",
        "|---|---|---|---:|---:|---:|---:|---:|---|",
    ]
    for (n, p), r in results.items():
        mine = [round(r["paired"] * 100), round(r["pool"] * 100, 1), round(r["junior"] * 100, 1),
                round(r["loss"] * 100), round(r["p5"] * 100)]
        ref = readme.get((n, p))
        if ref is None:
            status = "non publiée"
        elif all(abs(a - b) < 1e-9 for a, b in zip(mine, ref)):
            status = "✅ identique"
        else:
            status = "❌ " + " / ".join(f"{b:g}" for b in ref)
            mismatches.append(((n, p), mine, ref))
        lines.append(f"| {n} | {n * 10} k$ | {p} % | {pct(r['paired'], 0)} | {pct(r['pool'])} | {pct(r['junior'])} "
                     f"| {pct(r['loss'], 0)} | {pct(r['p5'], 0)} | {status} |")
    missing = sorted(set(readme) - set(results))
    lines += ["", f"Référence : trésorerie seule (92 % à 4 %) = {pct(0.92 * TBILL)} ; le senior touche {pct(SENIOR_RATE)}.", ""]
    if not readme:
        lines.append("⚠️ Tableau introuvable dans README.md : comparaison impossible.")
    elif mismatches or missing:
        lines.append(f"⚠️ {len(mismatches)} ligne(s) différente(s) du README, {len(missing)} ligne(s) du README non simulée(s).")
    else:
        lines.append(f"Les {len(readme)} lignes du README sont reproduites à l'identique (numpy {np.__version__}, graine 3).")

    rng = np.random.default_rng(11)
    lines += [
        "",
        "## 2. Sensibilité au netting et à Blend",
        "",
        "Pool de 1 M$ (800 k$ senior à 4,5 %, 200 k$ junior utilisé à 15 %), 560 k$ couverts en CETES, volume compensé "
        "par trimestre imposé. Blend à 6,5 % (hypothèse) plafonné à 20 % du capital, ou rien sur Blend (modèle de base du README).",
        "",
        "| Compensé / trimestre | Part compensée | Blend | Pool | Junior (moyenne) | Années perdantes | Pire année sur 20 |",
        "|---:|---:|---|---:|---:|---:|---:|",
    ]
    for net in (0, 0.5e6, 1e6, 2e6, 4e6):
        for blend_cap in (0.0, 0.20):
            r = netting_pool(rng, net, blend_cap)
            lines.append(f"| {net / 1e6:.1f} M$ | {pct(r['share'], 0)} | {'20 %' if blend_cap else 'non'} | {pct(r['pool'])} "
                         f"| {pct(r['junior'])} | {pct(r['loss'], 0)} | {pct(r['p5'], 0)} |".replace(".", ",", 1))
    lines += [
        "",
        "Lecture : sans volume compensé, le junior gagne à peine plus que les bons du Trésor pour un risque réel ; "
        "c'est la compensation entre importateurs et exportateurs qui le rémunère. Blend ajoute ~0,5 point.",
    ]
    style.OUT.mkdir(exist_ok=True)
    out = style.OUT / "tranches.md"
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return [out], mismatches, missing, len(readme)


if __name__ == "__main__":
    paths, mismatches, missing, n = main()
    print(paths[0])
    print(f"README : {n} lignes, {len(mismatches)} différentes, {len(missing)} non simulées")
