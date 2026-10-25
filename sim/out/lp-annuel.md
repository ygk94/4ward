# Rendement annuel d'un LP, pool sans tranches (exploration)

Généré par `sim/lp_annual.py`, taux en dur : USD 4 % / MXN 7 %. Ne pas éditer à la main. Non repris dans le README.

Pool de 500 k$, prix figé et spread du contrat (0,5 %), plafond 10 %, peso 13 %/an + chute de 15 % (3 % par an), capital libre au taux USD (4,0 %), réserve sur l'exposition nette.

| Scénario | Volume / trimestre (importateurs + exportateurs) | Marges placées | Moyenne | Médiane | Années perdantes | Pire année sur 20 | Pire trimestre possible |
|---|---|---|---:|---:|---:|---:|---:|
| Démarrage | 0,80 + 0,20 M$ | non | 7,8 % | 7,2 % | 31 % | −15,3 % | −11,1 % |
| Croisière | 3,00 + 1,50 M$ | non | 21,7 % | 16,9 % | 33 % | −36,9 % | −29,3 % |
| Croisière prudente (utilisation 15 %) | 2,62 + 1,88 M$ | non | 21,4 % | 20,9 % | 15 % | −10,8 % | −14,2 % |
| Équilibré | 3,25 + 2,75 M$ | non | 27,5 % | 28,1 % | 3 % | 3,0 % | −9,1 % |
| Saturé | 7,25 + 2,75 M$ | non | 46,9 % | −8,6 % | 52 % | −95,6 % | −89,9 % |
| Démarrage | 0,80 + 0,20 M$ | oui | 8,7 % | 8,1 % | 29 % | −14,5 % | −11,1 % |
| Croisière | 3,00 + 1,50 M$ | oui | 26,0 % | 21,1 % | 30 % | −34,2 % | −29,3 % |
| Croisière prudente (utilisation 15 %) | 2,62 + 1,88 M$ | oui | 25,6 % | 25,0 % | 11 % | −7,6 % | −14,2 % |
| Équilibré | 3,25 + 2,75 M$ | oui | 33,3 % | 34,0 % | 2 % | 7,8 % | −9,1 % |
| Saturé | 7,25 + 2,75 M$ | oui | 58,0 % | 0,4 % | 50 % | −94,2 % | −89,9 % |

Lecture : sans tranches, chaque LP porte tout le risque de change ; un pool saturé ou déséquilibré peut perdre une large part de son capital en un trimestre. D'où la tranche senior (taux fixe, protégée) et la tranche junior (première perte, plafonnée en utilisation) : voir tranches.md.
