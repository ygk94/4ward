# Rendement annuel d'un LP, pool sans tranches (exploration)

Généré par `sim/lp_annual.py`, taux Etherfuse : USD 3,6 % / MXN 5,55 %. Ne pas éditer à la main. Non repris dans le README.

Pool de 500 k$, prix figé et spread du contrat (0,5 %), plafond 10 %, peso 13 %/an + chute de 15 % (3 % par an), capital libre au taux USD (3,6 %), réserve sur l'exposition nette.

| Scénario | Volume / trimestre (importateurs + exportateurs) | Marges placées | Moyenne | Médiane | Années perdantes | Pire année sur 20 | Pire trimestre possible |
|---|---|---|---:|---:|---:|---:|---:|
| Démarrage | 0,80 + 0,20 M$ | non | 7,5 % | 6,8 % | 32 % | −15,6 % | −11,2 % |
| Croisière | 3,00 + 1,50 M$ | non | 21,3 % | 16,5 % | 34 % | −37,1 % | −29,4 % |
| Croisière prudente (utilisation 15 %) | 2,62 + 1,88 M$ | non | 21,1 % | 20,5 % | 15 % | −11,1 % | −14,2 % |
| Équilibré | 3,25 + 2,75 M$ | non | 27,1 % | 27,7 % | 3 % | 2,7 % | −9,2 % |
| Saturé | 7,25 + 2,75 M$ | non | 46,8 % | −8,7 % | 52 % | −95,6 % | −89,9 % |
| Démarrage | 0,80 + 0,20 M$ | oui | 8,3 % | 7,6 % | 30 % | −14,9 % | −11,2 % |
| Croisière | 3,00 + 1,50 M$ | oui | 25,3 % | 20,4 % | 30 % | −34,6 % | −29,4 % |
| Croisière prudente (utilisation 15 %) | 2,62 + 1,88 M$ | oui | 24,8 % | 24,2 % | 12 % | −8,3 % | −14,2 % |
| Équilibré | 3,25 + 2,75 M$ | oui | 32,3 % | 32,9 % | 2 % | 6,9 % | −9,2 % |
| Saturé | 7,25 + 2,75 M$ | oui | 56,8 % | −0,5 % | 50 % | −94,3 % | −89,9 % |

Lecture : sans tranches, chaque LP porte tout le risque de change ; un pool saturé ou déséquilibré peut perdre une large part de son capital en un trimestre. D'où la tranche senior (taux fixe, protégée) et la tranche junior (première perte, plafonnée en utilisation) : voir tranches.md.
