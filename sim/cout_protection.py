# Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
"""Coût de la protection pour la PME, comparé aux alternatives → out/cout-protection.png (+ .svg) et out/cout-protection.md.

Notre coût (calculé avec le contrat) : rémunération du pool (spread) + rendement perdu sur le dépôt (taux USD du pricing).
Option d'achat de pesos (protection non plafonnée) : prime Garman-Kohlhagen à la monnaie du taux figé, vol mesurée
(calibration.py, 5 ans) et vol du README (13 %). Banque et fintechs : ordres de grandeur, HYPOTHÈSES non sourcées.
"""
import math
from statistics import NormalDist

import calibration
import contract as c
import style
from style import plt

N01 = NormalDist()
T = c.REF["duration"] / c.SECS_PER_YEAR

# Hypothèses non vérifiées (mêmes ordres de grandeur que le README, « Ce que paie la PME »).
BANK_FORWARD = (0.005, 0.02)
FINTECHS = (0.005, 0.015)
README_VOL = 0.13


def our_cost():
    r = c.REF
    q = c.quote(r["notional"], r["spot"], r["r_usd"], r["r_dev"], r["spread"], r["duration"], r["cap"])
    no_spread = c.quote(r["notional"], r["spot"], r["r_usd"], r["r_dev"], 0, r["duration"], r["cap"])
    invoice = c.usdc(q["usdc_cost"])
    spread = c.usdc(q["usdc_cost"] - no_spread["usdc_cost"])
    lost_yield = c.usdc(q["margin"]) * r["r_usd"] / c.BPS * T
    return dict(invoice=invoice, spread=spread, lost_yield=lost_yield, margin=c.usdc(q["margin"]),
                forward=no_spread["forward_rate"] / c.PRICE_ONE)


def call_premium(vol):
    """Prime d'une option d'achat de MXN contre USD, prix d'exercice = taux figé sans spread, en % de la facture."""
    s = c.REF["spot"] / c.PRICE_ONE
    rd, rf = c.REF["r_usd"] / c.BPS, c.REF["r_dev"] / c.BPS
    f = s * math.exp((rd - rf) * T)
    k = f
    d1 = (math.log(f / k) + 0.5 * vol**2 * T) / (vol * math.sqrt(T))
    d2 = d1 - vol * math.sqrt(T)
    premium = math.exp(-rd * T) * (f * N01.cdf(d1) - k * N01.cdf(d2))
    return premium / k


def pct(x, dec=2):
    return f"{x * 100:.{dec}f} %".replace(".", ",")


def main():
    o = our_cost()
    ours = (o["spread"] + o["lost_yield"]) / o["invoice"]
    series = calibration.load()
    end = series["MXN"][-1][0]
    vol_mxn = calibration.realized_vol(series["MXN"], end.replace(year=end.year - 5))
    option_lo, option_hi = sorted((call_premium(vol_mxn), call_premium(README_VOL)))
    typical = vol_mxn * math.sqrt(T)

    # ---- graphique ----
    fig, ax = plt.subplots()
    labels = ["Notre protection", "Fintech de change\n(dépôt, KYC)", "Contrat à terme bancaire\n(ligne de crédit)",
              "Assurance non plafonnée\n(option d'achat)"]
    y = list(range(len(labels)))[::-1]
    ax.barh(y[0], o["spread"] / o["invoice"] * 100, color=style.BLUES[1], height=0.55, label="Rémunération du pool")
    ax.barh(y[0], o["lost_yield"] / o["invoice"] * 100, left=o["spread"] / o["invoice"] * 100, color=style.BLUES[0],
            height=0.55, label="Rendement perdu sur le dépôt")
    ax.text(ours * 100 + 0.06, y[0], f"{pct(ours, 1)}  ({c.fr(o['spread'] + o['lost_yield'], 0)} USDC)",
            va="center", fontsize=19, fontweight="bold", color=style.INK)
    for yy, (lo, hi), color in ((y[1], FINTECHS, style.GREY), (y[2], BANK_FORWARD, style.GREY),
                                (y[3], (option_lo, option_hi), style.ACCENT)):
        ax.barh(yy, (hi - lo) * 100, left=lo * 100, color=color, alpha=0.35, height=0.55)
        ax.barh(yy, lo * 100, color=color, alpha=0.12, height=0.55)
        ax.text(hi * 100 + 0.06, yy, f"{pct(lo, 1)} à {pct(hi, 1)}", va="center", fontsize=18, color=style.INK)
    ax.set_yticks(y, labels, fontsize=17, color=style.INK)
    ax.tick_params(axis="y", length=0)
    ax.grid(axis="y", visible=False)
    ax.set_xticks([0, 0.5, 1, 1.5, 2, 2.5, 3])
    ax.xaxis.set_major_formatter(style.percent(signed=False, dec=1))
    ax.set_xlim(0, 3.2)
    ax.set_xlabel("Coût pour couvrir une facture à 90 jours (% de la facture)")
    ax.set_title(f"La protection coûte {pct(ours, 1)} de la facture, ~{round(option_hi / ours)} fois moins qu'une option")
    ax.legend(loc="upper right", fontsize=15)
    fig.text(0.01, -0.06,
             f"Facture de 1 000 000 MXN (≈ {c.fr(o['invoice'], 0)} USDC), protection ±10 %, {c.rates_label()}. Option : prime calculée "
             f"(vol du peso {pct(vol_mxn, 0)} mesurée sur 5 ans, à 13 %).\nBanque et fintechs : ordres de grandeur du "
             "marché, non vérifiés devise par devise. Sans couverture : coût 0, mais un mouvement courant de "
             f"{pct(typical, 1)} coûte ≈ {c.fr(typical * o['invoice'], 0)} USDC.",
             fontsize=13, color=style.MUTED)
    paths = style.save(fig, "cout-protection")

    # ---- tableau ----
    lines = [
        "# Coût de la protection pour la PME",
        "",
        f"Généré par `sim/cout_protection.py`, {c.rates_label()}. Ne pas éditer à la main.",
        "",
        f"Facture de 1 000 000 MXN à 90 jours, prix figé **{c.fr(o['invoice'])} USDC**, protection ±10 % "
        f"(dépôt {c.fr(o['margin'])} USDC).",
        "",
        "| Poste | USDC | % de la facture | Source |",
        "|---|---:|---:|---|",
        f"| Rémunération du pool (spread 0,5 %) | {c.fr(o['spread'])} | {pct(o['spread'] / o['invoice'])} | contrat |",
        f"| Rendement perdu sur le dépôt ({pct(c.REF['r_usd'] / c.BPS, 1)}/an, 90 j) | {c.fr(o['lost_yield'])} | {pct(o['lost_yield'] / o['invoice'])} "
        "| contrat, taux USD du pricing |",
        f"| **Total** | **{c.fr(o['spread'] + o['lost_yield'])}** | **{pct(ours)}** (≈ {pct(ours / T, 1)}/an) | |",
        "",
        "| Solution | Coût pour 90 jours | Source |",
        "|---|---:|---|",
        f"| Aucune couverture | 0, mais un mouvement courant ({pct(typical, 1)}) coûte ≈ {c.fr(typical * o['invoice'], 0)} USDC "
        f"| vol MXN mesurée {pct(vol_mxn, 1)} (calibration.md) |",
        f"| **Notre protection** | **{pct(ours)}** | contrat |",
        f"| Contrat à terme bancaire | {pct(BANK_FORWARD[0], 1)} à {pct(BANK_FORWARD[1], 1)} | **hypothèse** (ordre de grandeur, non sourcé) |",
        f"| Fintechs de change (Ebury, iBanFirst, Corpay) | {pct(FINTECHS[0], 1)} à {pct(FINTECHS[1], 1)} | **hypothèse** (ordre de grandeur, non sourcé) |",
        f"| Option d'achat de pesos à la monnaie | {pct(option_lo)} à {pct(option_hi)} | calcul Garman-Kohlhagen, vol {pct(vol_mxn, 1)} "
        f"(mesurée) à {pct(README_VOL, 0)} (README) |",
        "",
        f"Une option à 13 % de vol coûte {pct(call_premium(README_VOL))} (le README dit ~2,6 %) : "
        f"≈ {c.fr(option_hi / ours, 1)} fois notre coût ; avec la vol mesurée, ≈ {c.fr(option_lo / ours, 1)} fois.",
        "La conversion USDC → MXN pour payer le fournisseur n'est pas comptée : elle est due avec ou sans couverture.",
    ]
    md = style.OUT / "cout-protection.md"
    md.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return paths + [md]


if __name__ == "__main__":
    for p in main():
        print(p)
