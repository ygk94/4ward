# Coût de la protection pour la PME

Généré par `sim/cout_protection.py`, taux Etherfuse : USD 3,6 % / MXN 5,55 %. Ne pas éditer à la main.

Facture de 1 000 000 MXN à 90 jours, prix figé **55 012,81 USDC**, protection ±10 % (dépôt 5 501,28 USDC).

| Poste | USDC | % de la facture | Source |
|---|---:|---:|---|
| Rémunération du pool (spread 0,5 %) | 273,70 | 0,50 % | contrat |
| Rendement perdu sur le dépôt (3,6 %/an, 90 j) | 48,83 | 0,09 % | contrat, taux USD du pricing |
| **Total** | **322,53** | **0,59 %** (≈ 2,4 %/an) | |

| Solution | Coût pour 90 jours | Source |
|---|---:|---|
| Aucune couverture | 0, mais un mouvement courant (5,4 %) coûte ≈ 2 998 USDC | vol MXN mesurée 11,0 % (calibration.md) |
| **Notre protection** | **0,59 %** | contrat |
| Contrat à terme bancaire | 0,5 % à 2,0 % | **hypothèse** (ordre de grandeur, non sourcé) |
| Fintechs de change (Ebury, iBanFirst, Corpay) | 0,5 % à 1,5 % | **hypothèse** (ordre de grandeur, non sourcé) |
| Option d'achat de pesos à la monnaie | 2,15 % à 2,55 % | calcul Garman-Kohlhagen, vol 11,0 % (mesurée) à 13 % (README) |

Une option à 13 % de vol coûte 2,55 % (le README dit ~2,6 %) : ≈ 4,4 fois notre coût ; avec la vol mesurée, ≈ 3,7 fois.
La conversion USDC → MXN pour payer le fournisseur n'est pas comptée : elle est due avec ou sans couverture.
