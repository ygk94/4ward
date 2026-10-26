# Script de la vidéo démo (3 min)

Pour : Hugo (enregistrement, depuis sa machine et son front). Rédigé par Paiement.
Voix off en anglais (jury international) ; indications de mise en scène en français.
Règle du council : zéro jargon à l'écran et à l'oral. On dit « lock the price », « protection », « deposit » ; jamais NDF, forward, collar, netting, bps.

Scénario : facture de **1 000 000 MXN**, protection **±10 %**, crash du peso **+12 %**.
Couverture à **90 jours** en **mode démo** (`set_time_factor 25920` : 90 jours annoncés → échéance réelle 5 min) : le prix affiché est le vrai prix à 90 jours.
Chiffres calculés avec le vrai Pricing (taux Etherfuse figés dans `scripts/rates.snapshot` : USD 3,60 %, MXN 5,55 % ; spread 0,5 %) sur un spot de **0,055** : **à recalculer avec le vrai devis** si le mock est aligné sur un autre spot Reflector (formules en bas).

---

## 1. Le problème — 0:00 → 0:30

| À l'écran | Voix off |
|---|---|
| Titre `<NOM>`, puis une facture : *Tequila Jalisco → Lucía, Lisbon — 1,000,000 MXN — due in 90 days* | *This is Lucía. She imports tequila into Lisbon, and she owes her supplier in Guadalajara one million pesos, due in 90 days.* |
| Graphique simple : le peso qui monte de 10 % → « +5,500 USD » en rouge | *If the peso rises 10% before then, that invoice costs her 5,500 dollars more. That's her whole margin.* |
| Logo banque barré | *A large company would call its bank and lock the exchange rate. Lucía has no credit line, and no bank will offer her that.* |
| Logo Stellar | *On Stellar, she can already pay abroad in seconds — but only at today's rate. Nothing lets her lock a rate for a payment in the future. Until now.* |

## 2. Bloquer le prix — 0:30 → 1:15

| À l'écran | Action | Voix off |
|---|---|---|
| `HedgeScreen` : montant 1,000,000, devise MXN, échéance, adresse du fournisseur | Hugo remplit le formulaire | *Lucía enters her invoice: one million pesos, the due date, and her supplier's address.* |
| `QuoteCard` : spot Reflector, prix figé **55,013 USDC** | le devis s'affiche | *The app reads the live peso rate from the Reflector oracle and gives her a locked price for 90 days: 55,013 USDC, whatever happens.* |
| Choix de la protection : **±10 %** → deposit **5,501.28 USDC** | Hugo choisit 10 % | *She picks her protection — plus or minus 10% — and deposits 10% of the invoice as a guarantee. That deposit is the most she can ever lose. No credit check, no margin calls.* |
| Fenêtre Freighter | **Signature n° 1** : « Lock my rate » (`open_hedge`) | *One signature in her wallet, and the price is locked.* |
| stellar.expert : la transaction, événement `hedge_opened` | plan de coupe 2-3 s | *It's on-chain: anyone can verify the locked rate and her deposit.* |

## 3. Le peso flambe — 1:15 → 1:45

| À l'écran | Action | Voix off |
|---|---|---|
| `CrashButton` : le peso monte de **+12 %** | Hugo clique (ou `scripts/crash.sh` en secours) | *Now let's say the peso jumps 12%.* |
| Comparaison côte à côte : **Without protection: 61,600 USDC (+6,587)** / **With <NOM>: 56,099 USDC (+1,086)** | | *Without protection, her invoice would cost 6,587 dollars more. With her protection, she only pays the part beyond her 10% limit: about 1,100 dollars.* |

## 4. Le jour J : règlement et paiement du fournisseur — 1:45 → 2:35

| À l'écran | Action | Voix off |
|---|---|---|
| Compte à rebours de l'échéance qui arrive à 0, mention discrète « Demo mode: 90 days run in 5 minutes » | coupe au montage pendant l'attente | *On the due date, anyone can trigger the settlement. It uses the rate at the due date, so nobody can pick a better moment.* |
| Bouton « Settle » → **+11,002.56 USDC** crédités | **Signature n° 2** (`settle`) | *The contract reads the oracle and pays her protection: her deposit back, plus 5,501.28 USDC from the pool.* |
| Bouton « Pay my supplier » → **1,000,000 MXN** envoyés | **Signature n° 3** (path payment) | *Then a Stellar path payment converts her dollars into pesos on the Stellar exchange and pays her supplier.* |
| stellar.expert : la page du path payment, **1,000,000 MXN reçus** par le fournisseur | plan de coupe 3-4 s, zoom sur le montant | *Her supplier receives exactly one million pesos — in his currency, in seconds.* |
| Récapitulatif : prix figé 55,013 · payé 56,099 · sans protection 61,600 | | *Two transactions today: settlement, then payment. Making them a single atomic step is on our roadmap.* |

## 5. Pourquoi ça tient, et la suite — 2:35 → 3:00

| À l'écran | Voix off |
|---|---|
| `PoolPanel` : capital, réservé, libre, indicateur « solvent » | *Behind Lucía, a pool of dollar liquidity takes the other side and earns the spread. It can't go bankrupt: it never accepts more protection than it can pay.* |
| Logos : Stellar · USDC · Reflector · Freighter | *Stablecoins, the Reflector oracle, path payments and Freighter: the protection and the payment run on the same rails. No bank, no credit line.* |
| Slide « Next » : exportateurs en contrepartie · rendement du capital libre · factures couvertes transférables · SDK pour wallets et anchors | *Next: matching importers with exporters, putting idle capital to work, and a "lock the rate" button for every Stellar wallet.* |
| `<NOM>` + phrase d'accroche | *<NOM>. Lock today's price for tomorrow's invoice.* |

---

## Avant d'enregistrer (checklist)

0. **Mode démo** : `set_time_factor --factor 25920` (fx-admin), une fois après le déploiement. Ouvrir la couverture avec `--duration_secs 7776000` (90 jours) : échéance réelle 5 min.
1. **Prix du mock à 0,055** : `set_price --asset '{"Other":"MXN"}' --price 5500000000000` (fx-admin ; pas `crash.sh --reset`, qui prend le cours Reflector, ~0,0533), daté de moins de 900 s avant `open_hedge`. Tous les chiffres de la voix off (55 013 USDC…) supposent ce spot.
2. Offre DEX alignée sur ce prix (`docs/payment-cli.md` § 5).
3. Pool financé (`deposit_lp`), Lucía avec assez d'USDC (prix figé + marge), fournisseur avec la trustline MXN.
4. **Le crash doit avoir lieu avant l'échéance réelle** (5 min après l'ouverture) : avec le fixing, `settle` prend le cours en vigueur à l'échéance, un crash posé après ne compte pas. Couper l'attente au montage.
5. Après le crash : **réaligner l'offre DEX sur +12 %** (fx-admin). `settle` utilise le cours en vigueur à l'échéance (fixing) : rien à rafraîchir avant `settle`, mais le crash doit dater de **moins de 15 min avant l'échéance** et `settle` doit partir peu après l'échéance (historique de Reflector ~12 h, du mock 64 prix : au-delà, erreur 312).
6. Freighter sur **Testnet**, compte `fx-lucia` importé ; fermer les autres onglets et notifications.
6 bis. (option) Lancer `./scripts/keeper.sh` dans un terminal avant d'ouvrir la couverture : `settle` part tout seul à l'échéance, la voix off du temps 4 peut dire *« On the due date, her protection settles itself — nobody has to click. »* et la signature n° 2 disparaît (il ne reste que le paiement du fournisseur). Si le keeper tourne, ne pas cliquer « Settle » dans le front (refus #201).
7. Garder en secours le parcours CLI (`docs/payment-cli.md`) : si un écran plante, terminal + stellar.expert pour la partie 4.

## Recalculer les chiffres avec le vrai devis

Avec `F` = prix figé par MXN (`quote.forward_rate`), `S0` = spot du jour, `S1 = S0 × 1,12` après le crash, `N` = 1 000 000 :

| Chiffre | Formule | Valeur (spot 0,055, 90 jours) |
|---|---|---|
| Prix figé | `N × F` (F = 0,0550128) | 55 012,81 |
| Deposit (±10 %) | `N × F × 10 %` | 5 501,28 |
| Sans protection | `N × S1` | 61 600 |
| Surcoût sans protection | `N × S1 − N × F` | 6 587,19 |
| Protection versée | `min(N × S1 − N × F, deposit)` | 5 501,28 |
| Crédité au règlement | `deposit + protection versée` | 11 002,56 |
| Payé au total avec `<NOM>` | `N × S1 − protection versée` | 56 098,72 |
| Surcoût avec `<NOM>` | `payé au total − prix figé` | 1 085,91 |

À vérifier après le recalcul : le mouvement du peso entre `F` et `S1` doit **dépasser 10 %**, pour que la démo montre bien le plafond (au-delà, Lucía ne paie que ce qui dépasse sa protection).
