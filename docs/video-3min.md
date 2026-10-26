# Vidéo 3 min : déroulé à valider

Consignes des mentors : 3 minutes, pas le deck dans la vidéo, il faut pitcher. On présente d'abord le problème, puis pourquoi Stellar, et le parcours seulement à la fin.

Ce qu'on voit :
- **Partie pitch (P1 à P5)** : des fonds plein écran aux couleurs du site (crème, encre, bleu #2f6bff, titres en Newsreader). Ils portent l'info clé de chaque phrase et ne remplacent pas le deck : ce sont des cartes HTML rendues en PNG, comme dans la méthode.
- **Partie démo (D1 à D6)** : le site en ligne `4ward-fx.vercel.app`, Freighter sur fx-lucia.

Méthode : `~/demo-video` (muette, puis prompteur, puis finale sur ta voix). Voix en anglais. Rythme ≈ 2,5 mots/s, cartes de 40 mots au plus.

Total : environ 340 mots, soit 2:15 de parole, plus le temps des actions à l'écran, pour finir vers 2:55.

---

## Partie 1 : pitch, avec des fonds (0:00 → 1:20)

| # | Temps | Fond (carte plein écran) | Texte à lire | Mots |
|---|---|---|---|---|
| P1 | 0:00 → 0:15 | Une facture : « Tequila Jalisco → Lucía, Lisbon · 1,000,000 MXN · due in 90 days ». En dessous, une courbe du peso qui monte, et « +$5,500 » en orange. | Lucía imports tequila into Lisbon. She owes her supplier in Guadalajara one million pesos, due in ninety days. If the peso rises ten percent before then, she pays fifty-five hundred dollars more. Her margin is gone. | 38 |
| P2 | 0:15 → 0:32 | Trois lignes barrées : « Bank forward · needs a credit line », « FX fintechs · deposit + KYC, few currencies », « Options · ~2.6% of the invoice ». En bas : « Most SMEs stay unhedged. » | Big companies lock the rate with a bank forward. Small businesses can't. Banks want a credit line, fintechs want deposits and paperwork, and options cost almost three percent. So most of them simply stay exposed. | 36 |
| P3 | 0:32 → 0:47 | Logo Stellar et trois pastilles « Native USDC », « Path payments », « Seconds, near-zero fees ». Dessous, en gros : « …but only at today’s rate. » | Stellar already moves money across borders in seconds, with native USDC and path payments. But only at today’s rate. Nothing lets a small business lock today’s rate for a payment it will make later. | 35 |
| P4 | 0:47 → 1:05 | Logo 4ward, puis trois étapes : « 1 · Lock a fair 90-day price », « 2 · Deposit a guarantee = max loss », « 3 · Settle at the due date, pay in pesos ». | 4ward does. Lucía locks a fair ninety-day price and deposits a small guarantee, which is also the most she can lose. At the due date the contract settles at that day’s rate, and a path payment pays her supplier in pesos. | 41 |
| P5 | 1:05 → 1:20 | Schéma : PME ↔ contrat Soroban ↔ pool de LPs, avec quatre logos ou étiquettes : « Reflector (SEP-40) », « Etherfuse rates », « USDC », « Path payments ». | It all runs on one Soroban contract: Reflector-compatible prices, Etherfuse rates inside the price, USDC and path payments. Liquidity providers back the hedges, and the contract never promises more than their capital covers. | 34 |

## Partie 2 : démo sur le site (1:20 → 2:55)

| # | Temps | Ce qu'on voit | Mises en avant | Texte à lire | Mots |
|---|---|---|---|---|---|
| D1 | 1:20 → 1:30 | Page Vaults : panneau du pool, onglets Safe / Dynamic | Entourer le TVL ; souligner « Safe · Dynamic » | Here is the live app on Stellar testnet. Liquidity providers deposit USDC into this vault and choose their risk: Safe or Dynamic. | 22 |
| D2 | 1:30 → 1:52 | « Hedge an invoice » : 1,000,000 MXN, 90 days, protection ±10 %. Le devis s'affiche, le globe trace l’arc USD → MXN. | Entourer **55,012.81 USDC** ; souligner « Protection ±10% » | Lucía enters her invoice: one million pesos in ninety days. The price comes from the oracle and the interest-rate gap: about fifty-five thousand dollars, locked within a ten percent protection. | 32 |
| D3 | 1:52 → 2:02 | Clic « Lock my price », fenêtre Freighter, signature (**coupée au montage**), message « Hedge #N opened » | Souligner « Hedge #N opened » | One signature in her wallet, and the price is locked on chain. | 12 |
| D4 | 2:02 → 2:14 | My positions : la couverture avec son compte à rebours. Incrustation « Peso +12% » (le crash est lancé hors champ) et bandeau « Demo mode: 90 days run in 5 minutes ». | Entourer le compte à rebours | Now the peso jumps twelve percent. In demo mode, ninety days run in five minutes. | 15 |
| D5 | 2:14 → 2:28 | Compte à rebours à 0 (**attente coupée**), clic Settle, signature, message « Settled: 11,002.56 USDC paid out » | Souligner le message de règlement | At the due date, the contract reads the rate at maturity, so nobody can game the timing, and pays out her protection. | 22 |
| D6 | 2:28 → 2:45 | « Pay my supplier », aperçu du coût sur le DEX, signature, récapitulatif « Your supplier received exactly 1,000,000 MXN … total cost 56,098.72 USDC » | Souligner « exactly 1,000,000 MXN » ; entourer le coût total | Then she pays her supplier from the app. He receives exactly one million pesos. Her total is fifty-six thousand dollars, instead of sixty-one thousand six hundred without protection. | 29 |
| D7 | 2:45 → 2:57 | Carte de clôture : logo 4ward, « Lock today’s price for tomorrow’s invoice », 4ward-fx.vercel.app · github.com/ygk94/4ward | — | 4ward: a fair, capped currency hedge for the businesses banks ignore, built on Stellar. Lock today’s price for tomorrow’s invoice. | 22 |

---

## Phrases à vérifier : est-ce qu'on promet plus que ce qui existe ?

| Phrase | Statut | Remarque |
|---|---|---|
| « options cost almost three percent » | estimation | 2,6 % vient de notre modèle (prime à la monnaie, volatilité 13 %), pas d'une cotation de marché. On peut dire « options cost a few percent » pour être prudent. |
| « Reflector-compatible prices » | ✅ exact | La démo lit un oracle mock de même interface. Ne pas dire « live Reflector prices ». |
| « Etherfuse rates inside the price » | ✅ exact | Ce sont des valeurs figées (snapshot du 25/10) poussées par l'admin. |
| « never promises more than their capital covers » | ✅ exact | Règle de capacité : réservé ≤ capital Dynamic. |
| « nobody can game the timing » | ✅ exact | Le règlement se fait au fixing à l'échéance. |
| « 55,012.81 », « 11,002.56 », « 56,098.72 », « 61,600 » | ✅ mesurés | À condition de remettre le mock à 0,055 et l'offre DEX à 0,055 avant la prise, puis à 0,0616 après le crash. |
| « He receives exactly one million pesos » | ✅ exact | Path payment strict receive. |

## Contraintes de la prise

- **Signatures Freighter (3, toutes à faire par toi)** : Lock my price, Settle, Pay my supplier. Le script s'arrête et attend à chaque fois, et on coupe au montage.
- **Attente de l'échéance** : 5 minutes réelles entre D3 et D5. On filme le compte à rebours, puis on coupe. Le crash (`crash.sh 12` puis l'offre DEX à 0,0616) se lance hors champ pendant ce temps, avant l'échéance.
- **Avant chaque prise** : `crash.sh --demo MXN`, offre DEX à 0,055, clés `fx.*` vidées du localStorage, keepalive actif, Ne pas déranger.
- **Après la vidéo** : `seed-testnet.sh`, puis keepalive avec `--keeper`, pour que les jurés trouvent un site plein.
