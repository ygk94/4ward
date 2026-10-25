# sim/ : chiffres et graphiques Pricing pour le deck

Tout se régénère en une commande, à l'identique d'un lancement à l'autre (graines fixes, cours historiques versionnés).

## Lancer

```sh
# une fois : venv hors du dépôt
python3 -m venv ~/.venvs/fistea-sim
~/.venvs/fistea-sim/bin/pip install -r sim/requirements.txt

# à chaque fois
~/.venvs/fistea-sim/bin/python sim/run_all.py

# si l'équipe adopte les taux Etherfuse (USD 3,60 % / MXN 5,55 % / BRL 11,48 % / EUR 2,10 %, ARS inchangé)
~/.venvs/fistea-sim/bin/python sim/run_all.py --rates etherfuse
```

`--rates` choisit le jeu de taux de la démo (`contract.RATE_SETS`) : `en-dur` par défaut (`scripts/setup-testnet.sh`),
`etherfuse` (`scripts/sync-rates.sh`). Il réécrit `out/` (chiffres de la démo, courbe, coût de la protection, LP) ;
l'auto-test du contrat reste sur les taux en dur, les tranches (modèle du README) et la calibration n'en dépendent pas.
Avec les taux Etherfuse, la démo à 0,055 donne : prix figé 55 012,81 · avec protection 56 098,72 (+1 085,90).

`run_all.py` s'arrête si le portage du contrat ne retrouve pas les valeurs de référence, et sort en erreur si
les tranches ne reproduisent plus le tableau du README. Il liste aussi les chiffres périmés trouvés dans les textes.

Rafraîchir les cours historiques (facultatif, change les chiffres de calibration) : `python sim/fetch_fx.py [AAAA-MM-JJ]`.

## Scripts

| Script | Produit | Contenu |
|---|---|---|
| `contract.py` | (auto-test) | Portage exact en entiers de `pricing.rs` (forward, spread, `to_usdc`, marge) et de `hedge.rs` (`pnl` borné), constantes de `vault.rs` |
| `pnl.py` | `out/cout-total.png` / `.svg` | Coût total de la facture selon le mouvement du peso (−20 % à +20 %), sans protection et avec ±5 / 10 / 20 %, point de la démo |
| `demo.py` | `out/chiffres-demo.md` | Chiffres de la démo à 0,055, aligné sur Reflector (0,05325), et avec les taux Etherfuse ; contrôle des chiffres cités dans les textes |
| `calibration.py` | `out/calibration.md` | Vol mesurée (1 an, 5 ans), probabilité de dépasser le plafond minimum, versement moyen du pool vs spread |
| `tranches.py` | `out/tranches.md` | Tableau senior / junior du README (reproduit à l'identique), sensibilité au netting et à Blend |
| `lp_annual.py` | `out/lp-annuel.md` | Exploration : rendement d'un LP dans un pool sans tranches (non repris dans le README) |
| `cout_protection.py` | `out/cout-protection.png` / `.svg` / `.md` | Coût de la protection (0,60 %) vs banque, fintechs, option |
| `fetch_fx.py` | `data/*.csv` | Cours BCE (MXN, BRL, EUR) et dollar officiel argentin, 5 ans (sources : `data/SOURCES.md`) |
| `style.py` | | Style commun des graphiques (16:9, grosses polices) |

`tranches.py` et `lp_annual.py` reprennent les simulations faites pendant la discussion « économie du pool »
(`small_pool.py`, `tranches2.py` + `netting.py`, `lp_annual.py`), nettoyées et branchées sur les constantes du contrat.

## Chiffres clés pour les slides

Facture de 1 000 000 MXN à 90 jours, spot 0,055, taux en dur (USD 4 % / MXN 7 %), spread 0,5 %, protection ±10 % :

| | |
|---|---|
| Prix figé | **54 873,05 USDC** (taux 0,0548731 au lieu de 0,055 au comptant) |
| Dépôt | 5 487,31 USDC |
| Peso +12 %, sans protection | 61 600 USDC (**+6 727**) |
| Peso +12 %, avec protection | 56 112,69 USDC (**+1 240**) : Lucía ne paie que ce qui dépasse ses 10 % |
| Coût de la protection | **327 USDC = 0,60 %** de la facture (273 pour le pool + 54 de rendement perdu sur le dépôt) |
| Option équivalente non plafonnée | 2,2 à 2,5 % : **~4 fois plus cher** |
| Vol du peso mesurée | 8 % sur un an, 11 % sur 5 ans : mouvement courant en 90 jours ≈ 5,4 % (≈ 3 000 USDC sur la facture) |
| Junior (pool de 500 k$, 80 contrats de 10 k$ par trimestre) | ≈ 13,8 % par an ; senior 4,5 % fixe |

Si le mock est aligné sur Reflector (0,05325) : prix figé 53 127,09 · sans protection 59 640 (+6 513) ·
avec 54 327,29 (+1 200) ; voir `out/chiffres-demo.md`.

## Hypothèses

- **Contrat** : formules et arrondis identiques au contrat ; le paiement du fournisseur se fait au cours de l'oracle
  (offre DEX alignée), sans frais de conversion.
- **Calibration** : vol réalisée (pas implicite), normale sur 90 jours, taux figé non biaisé ; ne capte pas les
  dévaluations (ARS 12/2023). ARS = dollar officiel Banco Nación (cours de détail).
- **Tranches et LP** : peso à 13 %/an avec sauts de 15 % (au-dessus de la vol mesurée : prudent), spread de
  0,4 % par contrat dont 20 % pour le protocole (le contrat prend 0,5 %), bons du Trésor à 4 %, 8 % en USDC,
  couverture en CETES au taux USD du pricing, conversion 0,3 %/an, utilisation du junior plafonnée à 15 %.
  Le netting et la couverture en CETES ne sont pas dans le MVP (roadmap).
- **Coût de la protection** : rendement perdu au taux USD du pricing (4 %) ; option valorisée par Garman-Kohlhagen
  à la monnaie du taux figé. **Banque (0,5-2 %) et fintechs (0,5-1,5 %) : ordres de grandeur non sourcés.**

## Pour le deck (Hugo)

| Image | Slide | Commentaire (FR) | Voice-over (EN) |
|---|---|---|---|
| `out/cout-total.png` (ou `.svg`) | « Une protection à perte maximale connue » | Quoi qu'il arrive au peso, la facture de Lucía reste dans une bande autour du prix figé : à +12 %, elle paie 56 113 USDC au lieu de 61 600. | *Whatever the peso does, Lucía's invoice stays close to the locked price: after a 12% jump she pays 56,113 USDC instead of 61,600.* |
| `out/cout-protection.png` (ou `.svg`) | « Ce que paie la PME » | La protection coûte 0,6 % de la facture, au prix d'une banque, sans ligne de crédit, et environ quatre fois moins qu'une assurance non plafonnée. | *Protection costs 0.6% of the invoice: bank pricing without a credit line, and about four times cheaper than unlimited insurance.* |

Tableaux à recopier si besoin : `out/chiffres-demo.md` (démo), `out/tranches.md` (senior / junior),
`out/calibration.md` (réponse à « pourquoi 5 % minimum sur le peso ? »).
