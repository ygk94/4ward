"""Coût total de la facture selon le mouvement du peso, sans et avec protection → out/cout-total.png (+ .svg).

Facture de la démo : 1 000 000 MXN à 90 jours, spot 0,055, jeu de taux actif (en dur par défaut), spread 0,5 %.
Chaque point est calculé avec les formules entières du contrat (contract.py) : prix figé, marge, P&L borné.
Coût total = achat des pesos au cours de l'échéance − protection versée par le pool (ou + prise sur la marge).
"""
import numpy as np

import contract as c
import style
from style import plt

CAPS = (500, 1_000, 2_000)
MOVES_BPS = range(-2_000, 2_001, 25)   # −20 % → +20 %, pas de 0,25 %


def curve(cap=None):
    """Coût total en USDC pour chaque mouvement ; cap=None : sans protection."""
    q = c.quote(c.REF["notional"], c.REF["spot"], c.REF["r_usd"], c.REF["r_dev"], c.REF["spread"],
                c.REF["duration"], cap or c.REF["cap"])
    out = []
    for m in MOVES_BPS:
        s = c.settle_buy(c.REF["notional"], q, c.bump(c.REF["spot"], m))
        out.append(c.usdc(s["unprotected"] if cap is None else s["protected"]))
    return np.array(out), q


def main():
    x = np.array(MOVES_BPS) / 100
    fig, ax = plt.subplots()

    y0, _ = curve(None)
    ax.plot(x, y0, color=style.GREY, lw=3.5, ls="--", label="Sans protection", zorder=2)
    for cap, color in zip(CAPS, style.BLUES):
        y, q = curve(cap)
        ax.plot(x, y, color=color, lw=4, label=f"Protection ±{cap // 100} %", zorder=3)
    locked = c.usdc(q["usdc_cost"])
    ax.axhline(locked, color=style.MUTED, lw=1.2, ls=":", zorder=1)
    ax.text(-19.6, locked + 400, f"Prix figé : {c.fr(locked, 0)} USDC", color=style.MUTED, fontsize=15, va="bottom")

    # point de la démo : peso +12 %, protection ±10 %
    s = c.scenario()
    paid, without = c.usdc(s["protected"]), c.usdc(s["unprotected"])
    ax.scatter([12, 12], [without, paid], s=[110, 170], color=[style.GREY, style.ACCENT], zorder=5)
    ax.annotate(f"Sans protection : {c.fr(without, 0)}\n(+{c.fr(without - locked, 0)} USDC)",
                xy=(12, without), xytext=(1.5, 64_200), color=style.INK, fontsize=16,
                arrowprops=dict(arrowstyle="-", color=style.MUTED, lw=1.2))
    ax.annotate(f"Démo : peso +12 %, protection ±10 %\nLucía paie {c.fr(paid, 0)} USDC (+{c.fr(paid - locked, 0)})",
                xy=(12, paid), xytext=(2.5, 47_300), color=style.ACCENT, fontsize=16, fontweight="bold",
                arrowprops=dict(arrowstyle="-", color=style.ACCENT, lw=1.5))

    ax.set_title("Avec la protection, le coût de la facture reste borné")
    ax.set_xlabel("Mouvement du peso face au dollar d'ici l'échéance")
    ax.set_ylabel("Coût total de la facture (USDC)")
    ax.xaxis.set_major_formatter(style.percent())
    ax.yaxis.set_major_formatter(style.thousands())
    ax.set_xlim(-20, 20)
    ax.set_ylim(42_000, 68_000)
    ax.legend(loc="upper left", bbox_to_anchor=(0, 0.93))
    fig.text(0.01, -0.04, "Facture de 1 000 000 MXN à 90 jours, 1 MXN = 0,055 USD à l'ouverture, "
             f"{c.rates_label()}. Calcul identique au contrat (frais de change du jour J non comptés).",
             fontsize=13, color=style.MUTED)
    return style.save(fig, "cout-total")


if __name__ == "__main__":
    for p in main():
        print(p)
