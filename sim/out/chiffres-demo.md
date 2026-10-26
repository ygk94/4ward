# Chiffres de la démo

Généré par `sim/demo.py` (formules entières du contrat, `sim/contract.py`), taux Etherfuse : USD 3,6 % / MXN 5,55 %. Ne pas éditer à la main.

Facture de **1 000 000 MXN à 90 jours**, spread 0,5 %, protection ±10 %, crash du peso **+12 %** sur le spot (même calcul que `scripts/crash.sh`) avant l'échéance. Le paiement du fournisseur est supposé au cours de l'oracle (offre DEX alignée).

| | Vidéo : spot 0,055, taux Etherfuse : USD 3,6 % / MXN 5,55 % | Mock aligné sur Reflector : 0,05325, taux Etherfuse : USD 3,6 % / MXN 5,55 % | Reflector, taux en dur : USD 4 % / MXN 7 % |
|---|---:|---:|---:|
| Cours du peso à l'ouverture (USD) | 0,05500 | 0,05325 | 0,05325 |
| Taux figé (USD par MXN) | 0,0550128 | 0,0532624 | 0,0531271 |
| **Prix figé** (USDC) | **55 012,81** | **53 262,41** | **53 127,09** |
| Dépôt, protection ±10 % (USDC) | 5 501,28 | 5 326,24 | 5 312,71 |
| Cours après +12 % (USD) | 0,06160 | 0,05964 | 0,05964 |
| **Sans protection** : coût de la facture | **61 600,00** | **59 640,00** | **59 640,00** |
| Surcoût sans protection vs prix figé | +6 587,19 | +6 377,59 | +6 512,91 |
| Protection versée par le pool | 5 501,28 | 5 326,24 | 5 312,71 |
| Crédité au règlement (dépôt + protection) | 11 002,56 | 10 652,48 | 10 625,42 |
| **Avec protection** : payé au total | **56 098,72** | **54 313,76** | **54 327,29** |
| Surcoût avec protection vs prix figé | +1 085,90 | +1 051,35 | +1 200,20 |
| Mouvement entre taux figé et cours final | +11,97 % | +11,97 % | +12,26 % |

Lecture : avec la protection, Lucía ne paie que ce qui dépasse ses 10 % ; sans, elle paie tout le mouvement.
La 2ᵉ colonne sert si le mock est réaligné sur Reflector avant l'enregistrement (`scripts/crash.sh --reset`) ; la 3ᵉ compare avec l'autre jeu de taux (les taux par défaut sont ceux de `scripts/rates.snapshot`, poussés sur le pool déployé ; `python sim/run_all.py --rates en-dur` régénère tout avec les anciens taux 400/700).

## Cohérence avec les textes

Chiffres de la vidéo (taux Etherfuse : USD 3,6 % / MXN 5,55 %) : prix figé 55 012,81 · dépôt 5 501,28 · sans protection 61 600,00 (+6 587,19) · avec 56 098,72 (+1 085,90) · crédité 11 002,56.

Aucune mention périmée trouvée dans README.md, docs/video-script.md, TODO.md, docs/memo-front-pricing.md, sim/README.md, scripts/crash.sh, front/src/home/About.tsx.
