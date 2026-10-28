# Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
"""Rendement annuel d'un LP dans un pool SANS tranches (ex-lp_annual.py, exploration) → out/lp-annuel.md.

Pool de 500 k$, volumes d'importateurs (B) et d'exportateurs (V) imposés par scénario, 4 échéances de 90 jours
indépendantes. Prix figé et spread comme le contrat (jeu de taux actif, spread 0,5 % contre la PME), plafond 10 %.
Capital réservé = exposition NETTE × plafond (netting branché : roadmap ; le MVP réserve la somme brute des marges).
Capital libre en bons du Trésor à 4 % (pas de Blend dans le modèle de base) ; option : marges des PME placées aussi.
Non publié dans le README : sert à montrer pourquoi les tranches (le senior ne porte pas le risque de change).
"""
import numpy as np

import contract as c
import style

T = 90 / 365
SPREAD = c.REF["spread"] / c.BPS
CAP = c.REF["cap"] / c.BPS
VOL = 0.13            # hypothèse du README (vol mesurée : voir calibration.md)
JUMP_P = 0.03         # probabilité annuelle d'une chute brutale du peso
JUMP = 0.15
PATHS = 200_000

SCENARIOS = {          # TVL, importateurs B, exportateurs V (notionnels USD par trimestre)
    "Démarrage": (500e3, 0.8e6, 0.2e6),
    "Croisière": (500e3, 3e6, 1.5e6),
    "Croisière prudente (utilisation 15 %)": (500e3, 2.625e6, 1.875e6),
    "Équilibré": (500e3, 3.25e6, 2.75e6),
    "Saturé": (500e3, 7.25e6, 2.75e6),
}


def quarter(rng, tvl, b, v, margins_invested):
    """Résultat d'une échéance de 90 jours, en fraction du TVL."""
    r_usd, r_mxn = c.REF["r_usd"] / c.BPS, c.REF["r_dev"] / c.BPS   # jeu de taux actif ; capital libre au taux USD
    s0 = c.REF["spot"] / c.PRICE_ONE
    f = s0 * (1 + r_usd * T) / (1 + r_mxn * T)
    k_buy, k_sell = f * (1 + SPREAD), f * (1 - SPREAD)
    st = f * np.exp(-0.5 * VOL**2 * T + VOL * np.sqrt(T) * rng.standard_normal(PATHS))
    st = np.where(rng.random(PATHS) < 1 - np.exp(-JUMP_P * T), st / (1 + JUMP), st)
    pme_buy = np.clip(st / k_buy - 1, -CAP, CAP)      # gain des importateurs, en fraction du notionnel
    pme_sell = np.clip(1 - st / k_sell, -CAP, CAP)    # gain des exportateurs
    reserved = abs(b - v) * CAP
    idle = max(0.0, tvl - reserved) * r_usd * T + ((b + v) * CAP * r_usd * T if margins_invested else 0)
    return (-(pme_buy * b + pme_sell * v) + idle) / tvl


def pct(x, dec=1):
    return f"{x * 100:.{dec}f} %".replace(".", ",").replace("-", "−")


def main():
    rng = np.random.default_rng(2)
    lines = [
        "# Rendement annuel d'un LP, pool sans tranches (exploration)",
        "",
        f"Généré par `sim/lp_annual.py`, {c.rates_label()}. Ne pas éditer à la main. Non repris dans le README.",
        "",
        f"Pool de 500 k$, prix figé et spread du contrat ({pct(SPREAD)}), plafond {pct(CAP, 0)}, peso {pct(VOL, 0)}/an + "
        f"chute de {pct(JUMP, 0)} ({pct(JUMP_P, 0)} par an), capital libre au taux USD ({pct(c.REF['r_usd'] / c.BPS)}), réserve sur l'exposition nette.",
        "",
        "| Scénario | Volume / trimestre (importateurs + exportateurs) | Marges placées | Moyenne | Médiane "
        "| Années perdantes | Pire année sur 20 | Pire trimestre possible |",
        "|---|---|---|---:|---:|---:|---:|---:|",
    ]
    for invested in (False, True):
        for name, (tvl, b, v) in SCENARIOS.items():
            # trimestres composés ; un pool vidé ne repart pas (perte bornée à −100 %)
            year = np.prod([np.maximum(1 + quarter(rng, tvl, b, v, invested), 0) for _ in range(4)], axis=0) - 1
            worst_q = -abs(b - v) * CAP / tvl + max(0.0, tvl - abs(b - v) * CAP) * c.REF['r_usd'] / c.BPS * T / tvl
            lines.append(f"| {name} | {b / 1e6:.2f} + {v / 1e6:.2f} M$ | {'oui' if invested else 'non'} | {pct(year.mean())} "
                         f"| {pct(np.median(year))} | {pct((year < 0).mean(), 0)} | {pct(np.quantile(year, 0.05))} "
                         f"| {pct(worst_q)} |".replace(".", ",", 2))
    lines += [
        "",
        "Lecture : sans tranches, chaque LP porte tout le risque de change ; un pool saturé ou déséquilibré peut "
        "perdre une large part de son capital en un trimestre. D'où la tranche senior (taux fixe, protégée) et la "
        "tranche junior (première perte, plafonnée en utilisation) : voir tranches.md.",
    ]
    style.OUT.mkdir(exist_ok=True)
    out = style.OUT / "lp-annuel.md"
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return [out]


if __name__ == "__main__":
    print(main()[0])
