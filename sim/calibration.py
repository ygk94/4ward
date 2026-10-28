# Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
"""Calibration des plafonds minimums et du spread → out/calibration.md.

Pour MXN, BRL, ARS, EUR :
- volatilité annualisée réalisée sur 1 an et 5 ans, à partir des cours versionnés dans sim/data/ (fetch_fx.py) ;
  vol retenue = la plus haute des deux (prudent) ;
- modèle normal sur 90 jours : mouvement R ~ N(0, σ√t) autour du taux figé (le taux figé intègre déjà l'écart de taux) ;
- probabilité de dépasser le plafond minimum, et fréquence observée sur 5 ans (mouvement brut de 90 jours) ;
- côté pool, pour une couverture non compensée : versement moyen E[min(max(R, 0), plafond)] et écart-type de
  son résultat, comparés au spread de 0,5 %.
"""
import csv
import datetime as dt
import math
from pathlib import Path
from statistics import NormalDist

import contract as c
import style

DATA = Path(__file__).resolve().parent / "data"
N01 = NormalDist()
T = 90 / 365
TRADING_DAYS = 252
SPREAD = c.REF["spread"] / c.BPS

# Plafonds minimums posés par scripts/setup-testnet.sh (set_min_cap), en bps.
MIN_CAPS = {"MXN": 500, "BRL": 700, "ARS": 1_500, "EUR": 300}
NAMES = {"MXN": "peso mexicain", "BRL": "real brésilien", "ARS": "peso argentin", "EUR": "euro"}


def load():
    """{devise: [(date, devise par USD)]}, jours ouvrés seulement."""
    series = {k: [] for k in MIN_CAPS}
    with open(DATA / "ecb-usd.csv") as f:
        for row in csv.DictReader(f):
            d = dt.date.fromisoformat(row["date"])
            for k in ("MXN", "BRL", "EUR"):
                series[k].append((d, float(row[k])))
    with open(DATA / "ars-oficial.csv") as f:
        for row in csv.DictReader(f):
            d = dt.date.fromisoformat(row["date"])
            if d.weekday() < 5:   # ArgentinaDatos répète le cours le week-end
                series["ARS"].append((d, (float(row["compra"]) + float(row["venta"])) / 2))
    return series


def realized_vol(points, since):
    xs = [v for d, v in points if d >= since]
    logs = [math.log(b / a) for a, b in zip(xs, xs[1:])]
    m = sum(logs) / len(logs)
    return math.sqrt(sum((x - m) ** 2 for x in logs) / (len(logs) - 1) * TRADING_DAYS)


def observed_exceed(points, cap):
    """Part des fenêtres de 90 jours calendaires (5 ans) où |mouvement| > plafond."""
    dates = [d for d, _ in points]
    values = [v for _, v in points]
    hits = total = j = 0
    for i, (d, v) in enumerate(points):
        target = d + dt.timedelta(days=90)
        if target > dates[-1]:
            break
        j = max(j, i)
        while j + 1 < len(dates) and dates[j + 1] <= target:
            j += 1
        move = values[i] / values[j] - 1   # devise par USD → USD par devise : S_T / S_0 − 1
        total += 1
        hits += abs(move) > cap
    return hits / total


def max_daily_jump(points):
    """(date, hausse) du plus fort saut journalier du cours (devise par USD)."""
    i = max(range(1, len(points)), key=lambda i: points[i][1] / points[i - 1][1])
    return points[i][0], points[i][1] / points[i - 1][1] - 1


def collar_stats(s, cap):
    """R ~ N(0, s), résultat de la PME = clip(R, −cap, cap). (versement moyen du pool, écart-type, P(|R| > cap))."""
    k = cap / s
    tail = 1 - N01.cdf(k)
    payout = s * (N01.pdf(0) - N01.pdf(k)) + cap * tail          # E[clip(R, 0, cap)]
    var = s * s * ((2 * N01.cdf(k) - 1) - 2 * k * N01.pdf(k)) + cap * cap * 2 * tail
    return payout, math.sqrt(var), 2 * tail


def pct(x, dec=1):
    return f"{x * 100:.{dec}f} %".replace(".", ",")


def main():
    series = load()
    end = min(pts[-1][0] for pts in series.values())
    one_year = end - dt.timedelta(days=365)
    five_years = end.replace(year=end.year - 5)
    rows, notes = [], {}
    for cur, cap_bps in MIN_CAPS.items():
        pts = series[cur]
        v1, v5 = realized_vol(pts, one_year), realized_vol(pts, five_years)
        vol = max(v1, v5)
        s = vol * math.sqrt(T)
        cap = cap_bps / c.BPS
        payout, std, p_exceed = collar_stats(s, cap)
        payout10, std10, _ = collar_stats(s, 0.10)
        suggested = max(1, math.ceil(s * 100 - 0.25))   # ≈ 1 écart-type de 90 jours, arrondi au point
        rows.append(dict(cur=cur, v1=v1, v5=v5, vol=vol, s=s, cap=cap, p=p_exceed,
                         obs=observed_exceed([p for p in pts if p[0] >= five_years], cap),
                         payout=payout, std=std, payout10=payout10, std10=std10, suggested=suggested))
        notes[cur] = (pts[0][0], pts[-1][0])

    lines = [
        "# Calibration des plafonds minimums et du spread",
        "",
        "Généré par `sim/calibration.py` à partir de `sim/data/` (voir `sim/data/SOURCES.md`). Ne pas éditer à la main.",
        "",
        f"Horizon 90 jours (t = 90/365 an). Vol réalisée = écart-type des variations journalières (log), × √{TRADING_DAYS}. "
        f"Fenêtres : 1 an et 5 ans jusqu'au {end:%d/%m/%Y}. Modèle : mouvement autour du taux figé ~ normale(0, σ√t).",
        "",
        "## Plafonds minimums",
        "",
        "| Devise | Vol 1 an | Vol 5 ans | Vol retenue | Mouvement typique en 90 j (σ√t) | Plafond min actuel "
        "| P(dépasser) modèle | Fréquence observée (5 ans) | Plafond min ≈ 1 σ√t |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for r in rows:
        lines.append(
            f"| {r['cur']} ({NAMES[r['cur']]}) | {pct(r['v1'])} | {pct(r['v5'])} | **{pct(r['vol'])}** | {pct(r['s'])} "
            f"| {r['cap'] * 100:.0f} % | {pct(r['p'], 0)} | {pct(r['obs'], 0)} | {r['suggested']} % |")
    lines += [
        "",
        "Critère proposé : le plafond minimum couvre au moins **un mouvement typique de 90 jours** (≈ 2 chances sur 3 "
        "de rester dans la protection). En dessous, la PME croit être protégée alors qu'elle dépasse son plafond une "
        "fois sur deux. Le plafond minimum ne sert pas la solvabilité du pool : sa perte maximale reste la marge, "
        "quel que soit le plafond.",
        "",
        "## Spread : ce que le pool verse et ce qu'il encaisse",
        "",
        "Couverture d'un importateur **non compensée et non couverte** (pire cas pour le pool). Si le taux figé est "
        "juste (pas de tendance au-delà de l'écart de taux), le pool verse en moyenne autant qu'il reçoit : son "
        "espérance de gain est le spread, mais le résultat d'une couverture isolée varie beaucoup.",
        "",
        "| Devise | Plafond | Versement moyen du pool | Reçu moyen des PME | Spread encaissé | Écart-type du résultat "
        "| Spread pour gagner 4 trimestres sur 5 (isolé) |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    for r in rows:
        for cap, payout, std in ((r["cap"], r["payout"], r["std"]), (0.10, r["payout10"], r["std10"])):
            lines.append(
                f"| {r['cur']} | {cap * 100:.0f} % | {pct(payout, 2)} | {pct(payout, 2)} | {pct(SPREAD, 2)} | {pct(std, 2)} "
                f"| {pct(N01.inv_cdf(0.8) * std, 1)} |")
    by = {r["cur"]: r for r in rows}
    mxn, brl, ars, eur = by["MXN"], by["BRL"], by["ARS"], by["EUR"]
    ratios = [r["cap"] / r["s"] for r in rows]
    jump_day, jump = max_daily_jump(series["ARS"])
    lines += [
        "",
        "## Conclusion",
        "",
        f"Les plafonds minimums actuels sont **cohérents entre eux** (entre {min(ratios):.1f} et {max(ratios):.1f} "
        f"mouvement typique de 90 jours) ; ".replace(".", ",") +
        f"le BRL à {brl['cap'] * 100:.0f} % est au bon niveau ({pct(brl['p'], 0)} de chances de dépasser), le MXN à "
        f"{mxn['cap'] * 100:.0f} % est un peu juste ({pct(mxn['p'], 0)}, {mxn['suggested']} % suffirait) et l'EUR à "
        f"{eur['cap'] * 100:.0f} % aussi ({pct(eur['p'], 0)}, {eur['suggested']} % suggéré). L'ARS à 15 % couvre très largement la vol de l'année "
        f"écoulée ({pct(ars['v1'])}, bande de change encadrée), mais pas une dévaluation comme celle de décembre 2023 "
        f"(vol sur 5 ans {pct(ars['v5'])}) : le modèle normal ne la capte pas, la limite par devise doit rester serrée. "
        f"Le spread de 0,5 % **ne paie pas le risque** d'un carnet à sens unique (écart-type ≈ "
        f"{pct(mxn['std10'], 1)} par couverture MXN à 10 % ; il faudrait ≈ {pct(N01.inv_cdf(0.8) * mxn['std10'], 0)} pour "
        f"gagner 4 trimestres sur 5 sans compensation) : il rémunère le service une fois le risque **compensé "
        f"(importateurs contre exportateurs) puis couvert** (détention de la devise), comme le dit le README.",
        "",
        "## Hypothèses",
        "",
        f"- Normale sans queue épaisse ni saut : sous-estime les dévaluations (ARS le {jump_day:%d/%m/%Y} : "
        f"le dollar prend +{pct(jump, 0)} en un jour).",
        "- Taux figé non biaisé. En pratique, les devises à taux élevé baissent souvent moins que ne le prévoit "
        "l'écart de taux : les importateurs (côté acheteur de devise) gagneraient un peu plus souvent, au détriment "
        "du pool. Argument de plus pour la compensation et le skew de prix.",
        "- Fréquence observée : mouvement brut du cours sur 90 jours calendaires, sans l'écart de taux ; pour l'ARS, "
        "la dérive administrée (crawling peg avant 2024) gonfle le chiffre.",
        "- ARS : dollar officiel Banco Nación (cours de détail arrondi), pas le cours de gros du BCRA.",
        "- Vol retenue = max(1 an, 5 ans), vol réalisée et non implicite (pas de source de vol implicite gratuite).",
    ]
    for cur, (a, b) in notes.items():
        lines.append(f"- {cur} : {len(series[cur])} cours de {a:%d/%m/%Y} à {b:%d/%m/%Y}.")
    style.OUT.mkdir(exist_ok=True)
    out = style.OUT / "calibration.md"
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return [out]


if __name__ == "__main__":
    print(main()[0])
