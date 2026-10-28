# Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
"""Rafraîchit les cours historiques de sim/data/ (pas lancé par run_all : les CSV versionnés rendent tout reproductible).

  python sim/fetch_fx.py [AAAA-MM-JJ]     fin de la fenêtre (défaut : hier), 5 ans d'historique

Sources publiques, sans clé :
- MXN, BRL, EUR : cours de référence BCE, via Frankfurter (https://frankfurter.dev), en devise par USD, jours ouvrés.
- ARS : dollar officiel Banco Nación (compra / venta), via ArgentinaDatos
  (https://api.argentinadatos.com/v1/cotizaciones/dolares/oficial) ; la BCE ne publie plus l'ARS.
"""
import csv
import datetime as dt
import json
import sys
import urllib.request
from pathlib import Path

DATA = Path(__file__).resolve().parent / "data"
ECB_URL = "https://api.frankfurter.dev/v1/{start}..{end}?base=USD&symbols=MXN,BRL,EUR"
ARS_URL = "https://api.argentinadatos.com/v1/cotizaciones/dolares/oficial"


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "fistea-sim"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def main():
    end = dt.date.fromisoformat(sys.argv[1]) if len(sys.argv) > 1 else dt.date.today() - dt.timedelta(days=1)
    start = end.replace(year=end.year - 5)
    DATA.mkdir(exist_ok=True)

    ecb = get(ECB_URL.format(start=start, end=end))["rates"]
    with open(DATA / "ecb-usd.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["date", "MXN", "BRL", "EUR"])
        for day in sorted(ecb):
            w.writerow([day] + [ecb[day][k] for k in ("MXN", "BRL", "EUR")])

    ars = [x for x in get(ARS_URL) if start.isoformat() <= x["fecha"] <= end.isoformat()]
    with open(DATA / "ars-oficial.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["date", "compra", "venta"])
        for x in sorted(ars, key=lambda x: x["fecha"]):
            w.writerow([x["fecha"], x["compra"], x["venta"]])

    (DATA / "SOURCES.md").write_text(
        f"# Cours historiques\n\nRécupérés le {dt.date.today()} par `sim/fetch_fx.py`, fenêtre {start} → {end}.\n\n"
        f"- `ecb-usd.csv` : cours de référence BCE (devise par USD, jours ouvrés), via Frankfurter : {ECB_URL}\n"
        f"- `ars-oficial.csv` : dollar officiel Banco Nación (ARS par USD, compra / venta), via ArgentinaDatos : {ARS_URL}\n",
        encoding="utf-8")
    print(f"{len(ecb)} jours BCE, {len(ars)} jours ARS, {start} → {end}")


if __name__ == "__main__":
    main()
