# Script de la vidéo démo (3 min)

Pour : Hugo (enregistrement, depuis sa machine et son front). Rédigé par Paiement.
Voix off en anglais (jury international) ; indications de mise en scène en français.
Règle du council : zéro jargon à l'écran et à l'oral. On dit « lock the price », « protection », « deposit » ; jamais NDF, forward, collar, netting, bps.

Scénario : facture de **1 000 000 MXN**, protection **±10 %**, crash du peso **+12 %**.
Couverture à **90 jours** en **mode démo** (`set_time_factor 25920` : 90 jours annoncés → échéance réelle 5 min) : le prix affiché est le vrai prix à 90 jours.
Chiffres calculés avec le vrai Pricing (taux Etherfuse figés dans `scripts/rates.snapshot` : USD 3,60 %, MXN 5,55 % ; spread 0,5 %) sur un spot de **0,055** : le mock-oracle doit être **remis à 0,055** avant chaque prise (checklist, étape a) ; à défaut, recalculer avec le vrai devis (formules en bas).
Le front n'a ni bouton de crash, ni comparaison côte à côte, ni bouton « Pay my supplier », ni récapitulatif : ces plans passent par le **terminal** (crash, path payment) et par des **incrustations ajoutées au montage**.

---

## 1. Le problème — 0:00 → 0:35

| À l'écran | Voix off |
|---|---|
| Titre `<NOM>`, puis une facture : *Tequila Jalisco → Lucía, Lisbon — 1,000,000 MXN — due in 90 days* | *This is Lucía. She imports tequila into Lisbon, and she owes her supplier in Guadalajara one million pesos, due in 90 days.* |
| Graphique simple : le peso qui monte de 10 % → « +5,500 USD » en rouge | *If the peso rises 10% before then, that invoice costs her 5,500 dollars more.* |
| Logo banque barré | *A large company would call its bank and lock the exchange rate. Lucía has no credit line, and no bank will do it for her.* |
| Logo Stellar | *On Stellar, she can already pay abroad in seconds — but only at today's rate. Nothing lets her lock today's rate for a future payment. Until now.* |

## 2. Bloquer le prix — 0:35 → 1:15

| À l'écran | Action | Voix off |
|---|---|---|
| `HedgeScreen` : montant 1,000,000, devise MXN, échéance, adresse du fournisseur | Hugo remplit le formulaire | *Lucía enters her invoice: one million pesos, the due date, and her supplier's address.* |
| `QuoteCard` : « Today’s rate (demo oracle) », prix figé **55,012.81 USDC** | le devis s'affiche (chip « 90 days ») | *The app reads the peso rate from a Reflector-compatible price feed (a demo feed on testnet) and gives her a locked price for 90 days: 55,013 USDC, whatever happens.* |
| Choix de la protection : **±10 %** → deposit **5,501.28 USDC** | Hugo choisit 10 % | *She picks her protection — plus or minus 10% — and deposits 10% of the invoice as a guarantee. That deposit is the most she can ever lose. No credit check, no surprise bills.* |
| Fenêtre Freighter | **Signature n° 1** : « Lock my price » (`open_hedge`) | *One signature in her wallet, and the price is locked.* |
| stellar.expert : la transaction, événement `hedge_opened` | plan de coupe 2-3 s | *Anyone can check the locked rate and her deposit on Stellar.* |

## 3. Le peso flambe — 1:15 → 1:35

| À l'écran | Action | Voix off |
|---|---|---|
| Terminal : `./scripts/crash.sh 12` (0,055 → 0,0616), puis incrustation « Peso +12% » | terminal (fx-admin), **avant l'échéance réelle** (5 min après l'ouverture) | *Now let's say the peso jumps 12%.* |
| Incrustation ajoutée au montage, côte à côte : **Without protection: 61,600 USDC (+6,587)** / **With <NOM>: 56,099 USDC (+1,086)** | | *Without protection, her invoice would cost 6,587 dollars more. With her protection, she only pays the part beyond her 10% limit: about 1,086 dollars.* |

## 4. Le jour J : règlement et paiement du fournisseur — 1:35 → 2:30

| À l'écran | Action | Voix off |
|---|---|---|
| `Mes positions` : compte à rebours de l'échéance qui arrive à 0 ; incrustation « Demo mode: 90 days run in 5 minutes » ajoutée au montage | coupe au montage pendant l'attente (~10 s de plus après le 0, le temps que Settle apparaisse) | *On the due date, anyone can trigger the settlement. It uses the rate at the due date, so nobody can game the timing.* |
| Bouton « Settle » de `Mes positions` → **+11,002.56 USDC** crédités | **Signature n° 2** (`settle`) | *The contract reads the oracle and pays out her protection: her deposit back, plus 5,501.28 USDC from the pool.* |
| Terminal : path payment (`docs/payment-cli.md` § 6) → **1,000,000 MXN** envoyés | **Signature n° 3** (path payment, en CLI : le front n'a pas ce bouton) | *Then a Stellar path payment converts her dollars into pesos on the Stellar exchange and pays her supplier.* |
| stellar.expert : la page du path payment, **1,000,000 MXN reçus** par le fournisseur | plan de coupe 3-4 s, zoom sur le montant | *Her supplier receives exactly one million pesos — in his currency, in seconds.* |
| Incrustation ajoutée au montage, récapitulatif : prix figé 55,013 · payé 56,099 · sans protection 61,600 | | *Two transactions today: settlement, then payment. Making them one single click is on our roadmap.* |

## 5. Pourquoi ça tient, et la suite — 2:30 → 3:00

| À l'écran | Voix off |
|---|---|
| `VaultPage` : `PoolPanel` (capital, réservé, libre) + badge « Solvent » de `LpForm` | *Behind Lucía, a pool of dollars covers her and earns a small fee on every lock. It can't go bankrupt: it never promises more than it holds.* |
| Logos : Stellar · USDC · Reflector · Freighter | *Stablecoins, Reflector, path payments, Freighter: protection and payment on the same Stellar rails.* |
| Slide « Next » : exportateurs en contrepartie · rendement du capital libre · factures couvertes transférables · SDK pour wallets et anchors | *Next: matching importers with exporters, putting idle capital to work, and a "lock the rate" button for every Stellar wallet.* |
| `<NOM>` + phrase d'accroche | *<NOM>. Lock today's price for tomorrow's invoice.* |

---

## Avant d'enregistrer (checklist)

Une fois pour toutes :

0. **Mode démo** : `set_time_factor --factor 25920` (fx-admin), déjà fait sur le déploiement actuel. Ouvrir la couverture sur **90 days** (`--duration_secs 7776000`) : échéance réelle 5 min.
1. Mock déjà à jour (fonction `price` + historique) : **NE PAS redéployer**, ni relancer `setup-testnet.sh` (nouveaux IDs, `.env.testnet` et `front/.env.local` réécrits, couvertures perdues).
2. Pool financé (`deposit_lp`), Lucía avec assez d'USDC (prix figé + dépôt), fournisseur avec la trustline MXN. Le fournisseur a déjà des MXN (tests) : filmer la **variation** de son solde (+1 000 000 MXN), pas le solde absolu ; ne pas mettre en avant le prix de part LP.
3. Couvertures seed échues : lancer `./scripts/keeper.sh --once` pour les solder (sinon le keeper les règle une par une avant celle de la démo).
4. Freighter sur **Testnet**, compte `fx-lucia` importé ; fermer les autres onglets et notifications.

À chaque prise, dans l'ordre :

- (a) **Mock à 0,055** : `./scripts/crash.sh --demo` (ou à la main : `stellar contract invoke --id fx-mock-oracle --source-account fx-admin --network testnet -- set_price --asset '{"Other":"MXN"}' --price 5500000000000`), **moins de 900 s avant `open_hedge`** (sinon `./scripts/crash.sh --refresh` juste avant). Pas `crash.sh --reset` : il pose le cours Reflector (~0,0533) et tous les chiffres de la voix off supposent 0,055.
- (b) **Offre DEX à 0,055** (`docs/payment-cli.md` § 5, offre 931579, prix `11:200`).
- (c) `open_hedge` sur le front, chip « 90 days » ; vérifier **55,012.81 USDC** à l'écran.
- (d) `./scripts/crash.sh 12` (0,055 → 0,0616) **avant l'échéance réelle** (dans les 5 min) : avec le fixing, `settle` prend le cours en vigueur à l'échéance, un crash posé après ne compte pas. Le prix à l'échéance doit dater de moins de 900 s (le crash suffit).
- (e) **Offre DEX à 0,0616** (`docs/payment-cli.md` § 5).
- (f) `settle` : attendre **~10 s après que le compte à rebours atteint 0** (heure du ledger en retard ; couper au montage), puis Settle sur `Mes positions` (ou keeper, voir plus bas). `settle` doit partir peu après l'échéance (historique du mock 64 prix : au-delà, erreur 312).
- (g) Path payment du fournisseur (`docs/payment-cli.md` § 6), puis la page stellar.expert.

**Avant chaque nouvelle prise, refaire (a) et (b)** : `crash.sh 12` se cumule (0,0616 → 0,0690…).

Option keeper : lancer `./scripts/keeper.sh` dans un terminal avant (c) : `settle` part tout seul à l'échéance, la voix off du temps 4 peut dire *« On the due date, her protection settles itself — nobody has to click. »* et la signature n° 2 disparaît. Si le keeper règle la couverture de la démo, ne pas cliquer « Settle » (refus #201) ; `Mes positions` se recharge toutes les 8 s.

Secours : parcours CLI complet (`docs/payment-cli.md`) si un écran plante, terminal + stellar.expert pour la partie 4.

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
