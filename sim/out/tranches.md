# Tranches senior / junior

Généré par `sim/tranches.py`. Ne pas éditer à la main.

Constantes du contrat : senior 4,5 %, junior ≥ 20 % du capital, plafond 10 %. Hypothèses du modèle : peso 13 %/an + sauts de 15 %, spread 0,4 % par contrat de 90 j dont 20 % pour le protocole, 8 % en USDC, le reste en bons du Trésor à 4 %, pas de Blend.

## 1. Pool de démarrage (tableau du README)

500 k$ (400 k$ senior, 100 k$ junior utilisé à 15 % max), contrats de 10 k$ à 90 jours.

| Contrats / trimestre | Volume / trimestre | Part d'importateurs | Volume compensé | Pool | Junior (moyenne) | Années perdantes | Pire année sur 20 | README |
|---|---|---|---:|---:|---:|---:|---:|---|
| 20 | 200 k$ | 50 % | 82 % | 4,3 % | 3,7 % | 22 % | −5 % | ✅ identique |
| 20 | 200 k$ | 60 % | 76 % | 4,4 % | 3,8 % | 28 % | −8 % | non publiée |
| 20 | 200 k$ | 70 % | 59 % | 4,4 % | 3,8 % | 35 % | −13 % | ✅ identique |
| 40 | 400 k$ | 50 % | 87 % | 5,0 % | 7,1 % | 15 % | −5 % | ✅ identique |
| 40 | 400 k$ | 60 % | 79 % | 5,0 % | 7,1 % | 25 % | −11 % | non publiée |
| 40 | 400 k$ | 70 % | 60 % | 5,0 % | 6,9 % | 34 % | −19 % | ✅ identique |
| 80 | 800 k$ | 50 % | 91 % | 6,4 % | 13,8 % | 7 % | −2 % | ✅ identique |
| 80 | 800 k$ | 60 % | 80 % | 6,3 % | 13,6 % | 19 % | −11 % | ✅ identique |
| 80 | 800 k$ | 70 % | 60 % | 6,3 % | 13,3 % | 23 % | −16 % | ✅ identique |

Référence : trésorerie seule (92 % à 4 %) = 3,7 % ; le senior touche 4,5 %.

Les 7 lignes du README sont reproduites à l'identique (numpy 2.5.4, graine 3).

## 2. Sensibilité au netting et à Blend

Pool de 1 M$ (800 k$ senior à 4,5 %, 200 k$ junior utilisé à 15 %), 560 k$ couverts en CETES, volume compensé par trimestre imposé. Blend à 6,5 % (hypothèse) plafonné à 20 % du capital, ou rien sur Blend (modèle de base du README).

| Compensé / trimestre | Part compensée | Blend | Pool | Junior (moyenne) | Années perdantes | Pire année sur 20 |
|---:|---:|---|---:|---:|---:|---:|
| 0,0 M$ | 0 % | non | 4,9 % | 6,5 % | 36 % | −22 % |
| 0,0 M$ | 0 % | 20 % | 5,4 % | 9,0 % | 31 % | −20 % |
| 0,5 M$ | 37 % | non | 5,7 % | 10,7 % | 28 % | −19 % |
| 0,5 M$ | 37 % | 20 % | 6,3 % | 13,3 % | 23 % | −16 % |
| 1,0 M$ | 54 % | non | 6,6 % | 15,1 % | 20 % | −14 % |
| 1,0 M$ | 54 % | 20 % | 7,1 % | 17,5 % | 16 % | −12 % |
| 2,0 M$ | 70 % | non | 8,3 % | 23,5 % | 9 % | −5 % |
| 2,0 M$ | 70 % | 20 % | 8,8 % | 26,0 % | 7 % | −3 % |
| 4,0 M$ | 82 % | non | 11,6 % | 40,1 % | 1 % | 11 % |
| 4,0 M$ | 82 % | 20 % | 12,1 % | 42,7 % | 1 % | 14 % |

Lecture : sans volume compensé, le junior gagne à peine plus que les bons du Trésor pour un risque réel ; c'est la compensation entre importateurs et exportateurs qui le rémunère. Blend ajoute ~0,5 point.
