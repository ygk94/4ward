# Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
"""Style commun des graphiques du deck : 16:9, grosses polices, palette sobre, export PNG + SVG."""
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.ticker import FuncFormatter  # noqa: E402

OUT = Path(__file__).resolve().parent / "out"

INK = "#1f2933"       # texte
MUTED = "#7b8794"     # axes, notes
GRID = "#e4e7eb"
GREY = "#9aa5b1"      # « sans protection »
BLUES = ["#9ecae1", "#2b7bba", "#08306b"]  # protection ±5 / ±10 / ±20 %
ACCENT = "#d9480f"    # point de la démo
GOOD = "#2b8a3e"

plt.rcParams.update({
    "figure.figsize": (13.333, 7.5),
    "figure.dpi": 100,
    "savefig.dpi": 150,
    "font.size": 18,
    "axes.titlesize": 26,
    "axes.titleweight": "bold",
    "axes.titlelocation": "left",
    "axes.titlepad": 18,
    "axes.labelsize": 19,
    "axes.labelcolor": INK,
    "axes.edgecolor": MUTED,
    "axes.spines.top": False,
    "axes.spines.right": False,
    "axes.grid": True,
    "grid.color": GRID,
    "grid.linewidth": 1,
    "xtick.color": MUTED,
    "ytick.color": MUTED,
    "xtick.labelsize": 16,
    "ytick.labelsize": 16,
    "legend.fontsize": 17,
    "legend.frameon": False,
    "text.color": INK,
    "svg.fonttype": "none",
    "svg.hashsalt": "fistea",  # SVG identique d'un lancement à l'autre
})

METADATA = {"png": {"Software": None}, "svg": {"Date": None, "Creator": None}}


def thousands(dec=0):
    """Formateur d'axe à la française : 54 873 ; 1,5."""
    return FuncFormatter(lambda v, _: f"{v:,.{dec}f}".replace(",", " ").replace(".", ","))


def percent(signed=True, dec=0):
    def fmt(v, _):
        s = f"{v:+.{dec}f} %" if signed and v else f"{v:.{dec}f} %"
        return s.replace("-", "−").replace(".", ",")
    return FuncFormatter(fmt)


def save(fig, name):
    OUT.mkdir(exist_ok=True)
    paths = []
    for ext in ("png", "svg"):
        p = OUT / f"{name}.{ext}"
        fig.savefig(p, bbox_inches="tight", facecolor="white", metadata=METADATA[ext])
        paths.append(p)
    plt.close(fig)
    return paths
