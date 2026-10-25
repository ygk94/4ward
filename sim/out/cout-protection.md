# Coût de la protection pour la PME

Généré par `sim/cout_protection.py`, taux en dur : USD 4 % / MXN 7 %. Ne pas éditer à la main.

Facture de 1 000 000 MXN à 90 jours, prix figé **54 873,05 USDC**, protection ±10 % (dépôt 5 487,31 USDC).

| Poste | USDC | % de la facture | Source |
|---|---:|---:|---|
| Rémunération du pool (spread 0,5 %) | 273,00 | 0,50 % | contrat |
| Rendement perdu sur le dépôt (4,0 %/an, 90 j) | 54,12 | 0,10 % | contrat, taux USD du pricing |
| **Total** | **327,12** | **0,60 %** (≈ 2,4 %/an) | |

| Solution | Coût pour 90 jours | Source |
|---|---:|---|
| Aucune couverture | 0, mais un mouvement courant (5,4 %) coûte ≈ 2 990 USDC | vol MXN mesurée 11,0 % (calibration.md) |
| **Notre protection** | **0,60 %** | contrat |
| Contrat à terme bancaire | 0,5 % à 2,0 % | **hypothèse** (ordre de grandeur, non sourcé) |
| Fintechs de change (Ebury, iBanFirst, Corpay) | 0,5 % à 1,5 % | **hypothèse** (ordre de grandeur, non sourcé) |
| Option d'achat de pesos à la monnaie | 2,15 % à 2,55 % | calcul Garman-Kohlhagen, vol 11,0 % (mesurée) à 13 % (README) |

Une option à 13 % de vol coûte 2,55 % (le README dit ~2,6 %) : ≈ 4,3 fois notre coût ; avec la vol mesurée, ≈ 3,6 fois.
La conversion USDC → MXN pour payer le fournisseur n'est pas comptée : elle est due avec ou sans couverture.
