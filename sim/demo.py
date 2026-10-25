"""Chiffres de la démo (crash +12 %, protection ±10 %, 1 000 000 MXN à 90 jours) → out/chiffres-demo.md.

Trois colonnes : spot 0,055 (chiffres de la vidéo) et spot aligné sur Reflector (0,05325) avec le jeu de taux actif
(run_all.py --rates), puis spot Reflector avec l'autre jeu de taux, pour comparaison.
Vérifie aussi que les chiffres cités dans README.md et docs/video-script.md sont à jour.
"""
import re
from pathlib import Path

import contract as c
import style

ROOT = Path(__file__).resolve().parent.parent

def cases():
    other = next(name for name in c.RATE_SETS if name != c.RATES)
    rates = c.RATE_SETS[other]
    return [
        (f"Vidéo : spot 0,055, {c.rates_label()}", dict()),
        (f"Mock aligné sur Reflector : 0,05325, {c.rates_label()}", dict(spot=c.REFLECTOR_SPOT)),
        (f"Reflector, {c.rates_label(other)}", dict(spot=c.REFLECTOR_SPOT, r_usd=rates["USD"], r_dev=rates["MXN"])),
    ]


def rows(s):
    q = s["quote"]
    locked = c.usdc(q["usdc_cost"])
    return [
        ("Cours du peso à l'ouverture (USD)", f"{q['spot'] / c.PRICE_ONE:.5f}".replace(".", ",")),
        ("Taux figé (USD par MXN)", f"{q['forward_rate'] / c.PRICE_ONE:.7f}".replace(".", ",")),
        ("**Prix figé** (USDC)", f"**{c.fr(locked)}**"),
        ("Dépôt, protection ±10 % (USDC)", c.fr(c.usdc(q["margin"]))),
        ("Cours après +12 % (USD)", f"{s['spot_end'] / c.PRICE_ONE:.5f}".replace(".", ",")),
        ("**Sans protection** : coût de la facture", f"**{c.fr(c.usdc(s['unprotected']))}**"),
        ("Surcoût sans protection vs prix figé", "+" + c.fr(c.usdc(s["unprotected"]) - locked)),
        ("Protection versée par le pool", c.fr(c.usdc(s["pnl"]))),
        ("Crédité au règlement (dépôt + protection)", c.fr(c.usdc(s["payout"]))),
        ("**Avec protection** : payé au total", f"**{c.fr(c.usdc(s['protected']))}**"),
        ("Surcoût avec protection vs prix figé", "+" + c.fr(c.usdc(s["protected"]) - locked)),
        ("Mouvement entre taux figé et cours final", f"+{(s['spot_end'] / q['forward_rate'] - 1) * 100:.2f} %".replace(".", ",")),
    ]


# Chiffres périmés connus (anciens textes, 60 s ou approximations) → valeur juste à 90 jours.
STALE = [
    (r"6 ?600", "≈ +6 600 $ (ancienne approximation) au lieu de +6 727"),
    (r"1 ?100 ?\$|~1 ?100", "≈ 1 100 $ (ancienne approximation) au lieu de +1 240"),
    (r"55 ?275", "prix figé à 60 s (55 275) au lieu de 54 873,05 à 90 jours"),
    (r"56 ?072", "payé total à 60 s (56 072,50) au lieu de 56 112,69"),
    (r"5 ?527,50", "dépôt à 60 s (5 527,50) au lieu de 5 487,31"),
]


def stale_mentions():
    """(fichier, ligne, problème) pour les fichiers où les chiffres de la démo font foi."""
    found = []
    for rel in ("README.md", "docs/video-script.md", "TODO.md", "docs/memo-front-pricing.md"):
        path = ROOT / rel
        if not path.exists():
            continue
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for pat, why in STALE:
                if re.search(pat, line):
                    found.append((rel, n, why))
    return found


def main():
    results = [(label, c.scenario(**kw)) for label, kw in cases()]
    table = [rows(s) for _, s in results]
    lines = [
        "# Chiffres de la démo",
        "",
        f"Généré par `sim/demo.py` (formules entières du contrat, `sim/contract.py`), {c.rates_label()}. "
        "Ne pas éditer à la main.",
        "",
        "Facture de **1 000 000 MXN à 90 jours**, spread 0,5 %, protection ±10 %, crash du peso **+12 %** sur le spot "
        "(même calcul que `scripts/crash.sh`) avant l'échéance. Le paiement du fournisseur est supposé au cours de "
        "l'oracle (offre DEX alignée).",
        "",
        "| | " + " | ".join(label for label, _ in results) + " |",
        "|---|" + "---:|" * len(results),
    ]
    for i, (name, _) in enumerate(table[0]):
        lines.append(f"| {name} | " + " | ".join(t[i][1] for t in table) + " |")
    lines += [
        "",
        "Lecture : avec la protection, Lucía ne paie que ce qui dépasse ses 10 % ; sans, elle paie tout le mouvement.",
        "La 2ᵉ colonne sert si le mock est réaligné sur Reflector avant l'enregistrement (`scripts/crash.sh --reset`) ; "
        "la 3ᵉ compare avec l'autre jeu de taux (`scripts/sync-rates.sh` pousse les taux Etherfuse ; "
        "`python sim/run_all.py --rates etherfuse` régénère tout avec eux).",
        "",
        "## Cohérence avec les textes",
        "",
        "Chiffres de la vidéo avec les taux en dur (ceux du script vidéo) : prix figé 54 873,05 · dépôt 5 487,31 · "
        "sans protection 61 600 (+6 726,95) · avec 56 112,69 (+1 239,64) · crédité 10 974,61.",
        "",
    ]
    stale = stale_mentions()
    if stale:
        lines.append("Mentions périmées trouvées (à corriger par le propriétaire du fichier) :")
        lines.append("")
        lines += [f"- `{f}:{n}` : {why}" for f, n, why in stale]
    else:
        lines.append("Aucune mention périmée trouvée dans README.md, docs/video-script.md, TODO.md, docs/memo-front-pricing.md.")
    style.OUT.mkdir(exist_ok=True)
    out = style.OUT / "chiffres-demo.md"
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return [out], stale


if __name__ == "__main__":
    paths, stale = main()
    print(paths[0])
    for f, n, why in stale:
        print(f"  périmé : {f}:{n} {why}")
