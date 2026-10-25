# Chiffres de la démo

Généré par `sim/demo.py` (formules entières du contrat, `sim/contract.py`), taux en dur : USD 4 % / MXN 7 %. Ne pas éditer à la main.

Facture de **1 000 000 MXN à 90 jours**, spread 0,5 %, protection ±10 %, crash du peso **+12 %** sur le spot (même calcul que `scripts/crash.sh`) avant l'échéance. Le paiement du fournisseur est supposé au cours de l'oracle (offre DEX alignée).

| | Vidéo : spot 0,055, taux en dur : USD 4 % / MXN 7 % | Mock aligné sur Reflector : 0,05325, taux en dur : USD 4 % / MXN 7 % | Reflector, taux Etherfuse : USD 3,6 % / MXN 5,55 % |
|---|---:|---:|---:|
| Cours du peso à l'ouverture (USD) | 0,05500 | 0,05325 | 0,05325 |
| Taux figé (USD par MXN) | 0,0548731 | 0,0531271 | 0,0532624 |
| **Prix figé** (USDC) | **54 873,05** | **53 127,09** | **53 262,41** |
| Dépôt, protection ±10 % (USDC) | 5 487,31 | 5 312,71 | 5 326,24 |
| Cours après +12 % (USD) | 0,06160 | 0,05964 | 0,05964 |
| **Sans protection** : coût de la facture | **61 600,00** | **59 640,00** | **59 640,00** |
| Surcoût sans protection vs prix figé | +6 726,95 | +6 512,91 | +6 377,59 |
| Protection versée par le pool | 5 487,31 | 5 312,71 | 5 326,24 |
| Crédité au règlement (dépôt + protection) | 10 974,61 | 10 625,42 | 10 652,48 |
| **Avec protection** : payé au total | **56 112,69** | **54 327,29** | **54 313,76** |
| Surcoût avec protection vs prix figé | +1 239,64 | +1 200,20 | +1 051,35 |
| Mouvement entre taux figé et cours final | +12,26 % | +12,26 % | +11,97 % |

Lecture : avec la protection, Lucía ne paie que ce qui dépasse ses 10 % ; sans, elle paie tout le mouvement.
La 2ᵉ colonne sert si le mock est réaligné sur Reflector avant l'enregistrement (`scripts/crash.sh --reset`) ; la 3ᵉ compare avec l'autre jeu de taux (`scripts/sync-rates.sh` pousse les taux Etherfuse ; `python sim/run_all.py --rates etherfuse` régénère tout avec eux).

## Cohérence avec les textes

Chiffres de la vidéo avec les taux en dur (ceux du script vidéo) : prix figé 54 873,05 · dépôt 5 487,31 · sans protection 61 600 (+6 726,95) · avec 56 112,69 (+1 239,64) · crédité 10 974,61.

Mentions périmées trouvées (à corriger par le propriétaire du fichier) :

- `TODO.md:162` : prix figé à 60 s (55 275) au lieu de 54 873,05 à 90 jours
- `TODO.md:162` : payé total à 60 s (56 072,50) au lieu de 56 112,69
- `TODO.md:222` : ≈ +6 600 $ (ancienne approximation) au lieu de +6 727
- `TODO.md:222` : ≈ 1 100 $ (ancienne approximation) au lieu de +1 240
