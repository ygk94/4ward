# Calibration des plafonds minimums et du spread

Généré par `sim/calibration.py` à partir de `sim/data/` (voir `sim/data/SOURCES.md`). Ne pas éditer à la main.

Horizon 90 jours (t = 90/365 an). Vol réalisée = écart-type des variations journalières (log), × √252. Fenêtres : 1 an et 5 ans jusqu'au 23/10/2026. Modèle : mouvement autour du taux figé ~ normale(0, σ√t).

## Plafonds minimums

| Devise | Vol 1 an | Vol 5 ans | Vol retenue | Mouvement typique en 90 j (σ√t) | Plafond min actuel | P(dépasser) modèle | Fréquence observée (5 ans) | Plafond min ≈ 1 σ√t |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| MXN (peso mexicain) | 8,0 % | 11,0 % | **11,0 %** | 5,4 % | 5 % | 36 % | 29 % | 6 % |
| BRL (real brésilien) | 10,3 % | 13,2 % | **13,2 %** | 6,6 % | 7 % | 29 % | 15 % | 7 % |
| ARS (peso argentin) | 9,3 % | 35,7 % | **35,7 %** | 17,7 % | 15 % | 40 % | 25 % | 18 % |
| EUR (euro) | 5,4 % | 7,6 % | **7,6 %** | 3,8 % | 3 % | 43 % | 37 % | 4 % |

Critère proposé : le plafond minimum couvre au moins **un mouvement typique de 90 jours** (≈ 2 chances sur 3 de rester dans la protection). En dessous, la PME croit être protégée alors qu'elle dépasse son plafond une fois sur deux. Le plafond minimum ne sert pas la solvabilité du pool : sa perte maximale reste la marge, quel que soit le plafond.

## Spread : ce que le pool verse et ce qu'il encaisse

Couverture d'un importateur **non compensée et non couverte** (pire cas pour le pool). Si le taux figé est juste (pas de tendance au-delà de l'écart de taux), le pool verse en moyenne autant qu'il reçoit : son espérance de gain est le spread, mais le résultat d'une couverture isolée varie beaucoup.

| Devise | Plafond | Versement moyen du pool | Reçu moyen des PME | Spread encaissé | Écart-type du résultat | Spread pour gagner 4 trimestres sur 5 (isolé) |
|---|---:|---:|---:|---:|---:|---:|
| MXN | 5 % | 1,64 % | 1,64 % | 0,50 % | 3,71 % | 3,1 % |
| MXN | 10 % | 2,10 % | 2,10 % | 0,50 % | 5,13 % | 4,3 % |
| BRL | 7 % | 2,14 % | 2,14 % | 0,50 % | 4,90 % | 4,1 % |
| BRL | 10 % | 2,43 % | 2,43 % | 0,50 % | 5,82 % | 4,9 % |
| ARS | 15 % | 5,11 % | 5,11 % | 0,50 % | 11,42 % | 9,6 % |
| ARS | 10 % | 3,90 % | 3,90 % | 0,50 % | 8,42 % | 7,1 % |
| EUR | 3 % | 1,05 % | 1,05 % | 0,50 % | 2,33 % | 2,0 % |
| EUR | 10 % | 1,51 % | 1,51 % | 0,50 % | 3,77 % | 3,2 % |

## Conclusion

Les plafonds minimums actuels sont **cohérents entre eux** (entre 0,8 et 1,1 mouvement typique de 90 jours) ; le BRL à 7 % est au bon niveau (29 % de chances de dépasser), le MXN à 5 % est un peu juste (36 %, 6 % suffirait) et l'EUR à 3 % aussi (43 %, 4 % suggéré). L'ARS à 15 % couvre très largement la vol de l'année écoulée (9,3 %, bande de change encadrée), mais pas une dévaluation comme celle de décembre 2023 (vol sur 5 ans 35,7 %) : le modèle normal ne la capte pas, la limite par devise doit rester serrée. Le spread de 0,5 % **ne paie pas le risque** d'un carnet à sens unique (écart-type ≈ 5,1 % par couverture MXN à 10 % ; il faudrait ≈ 4 % pour gagner 4 trimestres sur 5 sans compensation) : il rémunère le service une fois le risque **compensé (importateurs contre exportateurs) puis couvert** (détention de la devise), comme le dit le README.

## Hypothèses

- Normale sans queue épaisse ni saut : sous-estime les dévaluations (ARS le 13/12/2023 : le dollar prend +109 % en un jour).
- Taux figé non biaisé. En pratique, les devises à taux élevé baissent souvent moins que ne le prévoit l'écart de taux : les importateurs (côté acheteur de devise) gagneraient un peu plus souvent, au détriment du pool. Argument de plus pour la compensation et le skew de prix.
- Fréquence observée : mouvement brut du cours sur 90 jours calendaires, sans l'écart de taux ; pour l'ARS, la dérive administrée (crawling peg avant 2024) gonfle le chiffre.
- ARS : dollar officiel Banco Nación (cours de détail arrondi), pas le cours de gros du BCRA.
- Vol retenue = max(1 an, 5 ans), vol réalisée et non implicite (pas de source de vol implicite gratuite).
- MXN : 1282 cours de 22/10/2021 à 23/10/2026.
- BRL : 1282 cours de 22/10/2021 à 23/10/2026.
- ARS : 1305 cours de 25/10/2021 à 23/10/2026.
- EUR : 1282 cours de 22/10/2021 à 23/10/2026.
