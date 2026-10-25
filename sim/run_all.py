"""Régénère tout sim/out/ en une commande :  python sim/run_all.py [--rates en-dur|etherfuse]

--rates : jeu de taux d'intérêt de la démo (défaut : en-dur = scripts/setup-testnet.sh ; etherfuse = rendements
poussés par scripts/sync-rates.sh). Les tranches (modèle du README) et la calibration n'en dépendent pas.

S'arrête si le portage du contrat ne retrouve pas les valeurs de référence ; sort en erreur (code 1) si les
tranches ne reproduisent plus le tableau du README. Les cours historiques viennent de sim/data/ (fetch_fx.py).
"""
import argparse
import sys

try:
    import matplotlib  # noqa: F401
    import numpy as np
except ImportError:
    sys.exit("matplotlib / numpy introuvables. Venv hors du dépôt :\n"
             "  python3 -m venv ~/.venvs/fistea-sim && ~/.venvs/fistea-sim/bin/pip install -r sim/requirements.txt\n"
             "  ~/.venvs/fistea-sim/bin/python sim/run_all.py")

import calibration
import contract
import cout_protection
import demo
import lp_annual
import pnl
import style
import tranches


def main(argv=None):
    parser = argparse.ArgumentParser(description="Régénère sim/out/.")
    parser.add_argument("--rates", choices=sorted(contract.RATE_SETS), default="en-dur",
                        help="jeu de taux d'intérêt (défaut : en-dur)")
    args = parser.parse_args(argv)
    try:
        contract.self_test()
    except AssertionError as e:
        sys.exit(f"ARRÊT : {e}")
    print("contrat      : valeurs de référence retrouvées (forward 5487305413412, marge 54873054135, 56 112,69)")
    contract.use_rates(args.rates)
    print(f"taux         : {contract.rates_label()}")

    outputs = pnl.main()
    paths, stale = demo.main()
    outputs += paths
    outputs += calibration.main()
    paths, mismatches, missing, n_readme = tranches.main()
    outputs += paths
    outputs += lp_annual.main()
    outputs += cout_protection.main()

    for p in outputs:
        print(f"généré       : {p.relative_to(style.OUT.parent.parent)}")
    for f, line, why in stale:
        print(f"à corriger   : {f}:{line} {why}")
    if np.__version__ != "2.5.4":
        print(f"attention    : numpy {np.__version__} au lieu de 2.5.4, les tirages Monte-Carlo peuvent différer")
    if mismatches or missing or not n_readme:
        print(f"ÉCART README : {len(mismatches)} ligne(s) différente(s), {len(missing)} non simulée(s) (voir out/tranches.md)")
        return 1
    print(f"tranches     : les {n_readme} lignes du tableau du README sont reproduites")
    return 0


if __name__ == "__main__":
    sys.exit(main())
