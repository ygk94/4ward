# Parcours Paiement en ligne de commande

Le parcours de Lucía de bout en bout, sur le testnet : dépôt LP → `open_hedge` → crash → `settle` → réalignement DEX → path payment au fournisseur.
Validé le 25/10 sur le déploiement de Clément (liens en bas). Les commandes utilisent les **alias** (`fx-pool`, `fx-mock-oracle`, `fx-lucia`…) : elles se rejouent telles quelles sur n'importe quel déploiement fait par `./scripts/setup-testnet.sh`.

Conventions : montants en **stroops** (7 décimales : 1 USDC = `10000000`), prix de l'oracle en **14 décimales** (0,055 USD = `5500000000000`), plafond en **bps** (1000 = 10 %).

## 0. Préparation

```sh
cd fistea-hackmeridian
source .env.testnet          # ISSUER, USDC_SAC, POOL_ID, … du déploiement
```

**Prix du mock frais avant `open_hedge`.** `forward_rate` lit le spot, refusé s'il a plus de 900 s (#302). Relancer `set_price` **juste avant `open_hedge`**.
`settle` ne lit plus le spot mais le **fixing** : le cours en vigueur à l'échéance (`price(asset, maturity)` de l'oracle). Il faut donc un prix posé **dans les 900 s avant l'échéance** (le crash suffit), et rien à rafraîchir au moment du `settle`. Le mock doit être celui de `main` après le fixing (fonction `price` avec historique).

```sh
stellar contract invoke --id fx-mock-oracle --source-account fx-admin --network testnet -- \
  set_price --asset '{"Other":"MXN"}' --price 5500000000000          # 0,055 USD/MXN
```

## Mode démo : 90 jours en 5 minutes (fx-admin, une fois)

```sh
stellar contract invoke --id fx-pool --source-account fx-admin --network testnet --   set_time_factor --factor 25920                                      # échéance réelle = durée / 25 920
```
Le prix reste calculé sur la durée annoncée (`--duration_secs 7776000` = 90 jours → prix à 90 jours), l'échéance réelle tombe 300 s après l'ouverture. Événement `time_factor_set`. Lecture : `-- time_factor` (1 = temps réel).

## 1. Le LP remplit le pool (fx-lp)

```sh
stellar contract invoke --id fx-pool --source-account fx-lp --network testnet -- \
  deposit_lp --lp fx-lp --amount 1000000000000                        # 100 000 USDC
```
Retour : les parts du LP. Contrôle : `-- capacity` → `1000000000000`.

## 2. Lucía bloque son taux (fx-lucia)

```sh
stellar contract invoke --id fx-pool --source-account fx-lucia --network testnet -- \
  open_hedge --owner fx-lucia --supplier fx-supplier --currency MXN --side BuyCurrency \
  --notional 10000000000000 --duration_secs 7776000 --cap_bps 1000    # 1 000 000 MXN, 90 jours, 10 %
```
Retour : l'id de la couverture (`0` pour la première). Signature : **Lucía seule**.
Effets : transfert USDC Lucía → pool de la marge, événement `hedge_opened`, `capacity` baisse de la marge, `exposure MXN` augmente.

## 3. Crash du peso (fx-admin)

```sh
stellar contract invoke --id fx-mock-oracle --source-account fx-admin --network testnet -- \
  set_price --asset '{"Other":"MXN"}' --price 6160000000000          # +12 % : 0,0616
```
(+8 % : `5940000000000`.) Aucun mouvement d'argent. **À faire avant l'échéance réelle** : avec le fixing, `settle` prend le cours en vigueur à l'échéance.

## 4. Règlement à l'échéance (n'importe quel compte)

Attendre l'échéance réelle (`get_hedge` → champ `maturity`, en secondes Unix ; en mode démo, durée / facteur), puis :

```sh
stellar contract invoke --id fx-pool --source-account fx-supplier --network testnet -- \
  settle --hedge_id 0
```
Retour : l'USDC versé à Lucía (marge ± P&L, entre 0 et 2 × marge). Effets : transfert USDC pool → Lucía, événement `hedge_settled` (spot de fixing, P&L, versement).
Le cours utilisé est celui **en vigueur à l'échéance**, quel que soit le moment de l'appel : un prix posé après l'échéance ne compte pas. Régler peu après l'échéance : l'historique de Reflector ne remonte qu'à ~12 h, celui du mock à 64 prix (au-delà : #312).

### Règlement automatique : le keeper

Au lieu d'appeler `settle` à la main, `scripts/keeper.sh` surveille les couvertures et règle chacune dès son échéance (n'importe quel compte peut payer les frais, `fx-admin` par défaut) :

```sh
./scripts/keeper.sh                 # boucle, un passage toutes les 10 s (Ctrl+C pour arrêter)
./scripts/keeper.sh --once          # un seul passage
./scripts/keeper.sh --dry-run       # affiche ce qu'il réglerait, sans rien envoyer
POOL=fx-pool SOURCE=fx-admin ./scripts/keeper.sh --interval 5
```
Il parcourt les couvertures depuis l'id 0, compare leur échéance à l'heure du dernier ledger et affiche la raison d'un refus (#202 réseau en retard d'un ledger, #302, #312). Le lancer avant l'échéance règle aussi la limite d'historique de Reflector (~12 h) : le règlement part dans les secondes qui suivent l'échéance.
Testé sur testnet : couverture de 45 s réelles réglée par le keeper en boucle 10 s après l'échéance, sans intervention ([tx](https://stellar.expert/explorer/testnet/tx/656d79533ce6d192d21f3a5824722fc4ebc66a2bd202dcadcfb8f9993e490773)).

## 5. Réaligner l'offre DEX sur le nouveau prix (fx-admin)

Sans ça, le path payment se fait à l'ancien prix et le coût total de Lucía ne retombe plus sur le prix figé.

```sh
# id de l'offre MXN/USDC de fx-admin
curl -s "https://horizon-testnet.stellar.org/offers?seller=$ISSUER" | grep -o '"id": "[0-9]*"'

P=6160000000000                                  # même prix que le mock (14 décimales)
stellar tx new manage-sell-offer --source-account fx-admin --network testnet \
  --selling "MXN:$ISSUER" --buying "USDC:$ISSUER" --amount 1000000000000000 \
  --price "$((P / 10000000)):10000000" --offer-id <ID>                # 616000:10000000 = 0,0616
```
Le prix d'une offre Stellar est une fraction d'entiers 32 bits : `P / 10⁷ : 10⁷` garde 7 décimales et tient dans les bornes.

## 6. Paiement du fournisseur (fx-lucia)

```sh
stellar tx new path-payment-strict-receive --source-account fx-lucia --network testnet \
  --send-asset "USDC:$ISSUER" --send-max 619100000000 \
  --destination fx-supplier --dest-asset "MXN:$ISSUER" --dest-amount 10000000000000
```
*Strict receive* : le fournisseur reçoit **exactement** 1 000 000 MXN ; Lucía paie au plus `send-max`.
`send-max` = notionnel × spot + ~0,5 % de marge de glissement (61 600 → 61 910 USDC à +12 %, 59 400 → 59 500 à +8 %).

## Lectures (sans signature)

```sh
stellar contract invoke --id fx-pool --source-account fx-lp --network testnet -- get_hedge --hedge_id 0
stellar contract invoke --id fx-pool --source-account fx-lp --network testnet -- capacity
stellar contract invoke --id fx-pool --source-account fx-lp --network testnet -- total_assets
stellar contract invoke --id fx-pool --source-account fx-lp --network testnet -- exposure --currency MXN
stellar contract invoke --id fx-mock-oracle --source-account fx-lp --network testnet -- lastprice --asset '{"Other":"MXN"}'
stellar contract invoke --id $USDC_SAC --source-account fx-lp --network testnet -- balance --id fx-lucia
curl -s https://horizon-testnet.stellar.org/accounts/$(stellar keys address fx-supplier)   # soldes MXN / USDC
```

---

## Pour le front (`payment/HedgeScreen.tsx`)

### Enchaînement des appels

| Étape | Appel | Signe | Affichage après |
|---|---|---|---|
| Devis | `quote(currency, side, notional, duration_secs, cap_bps)` (Pricing) | — | spot, prix figé, marge |
| Bloquer mon taux | `open_hedge(...)` → id | Lucía (Freighter n° 1) | marge déposée, échéance, lien explorateur |
| Échéance | `settle(id)` → versement | Lucía ou n'importe qui (Freighter n° 2) | P&L, versement |
| Payer le fournisseur | `path-payment-strict-receive` (opération classique, `@stellar/stellar-sdk` `Operation.pathPaymentStrictReceive`) | Lucía (Freighter n° 3) | MXN livrés, coût total |

`settle` et le path payment sont **deux transactions** : une transaction Soroban ne peut pas contenir d'opération classique. Garder l'id de la couverture dans l'état du front (ou le relire dans l'événement `hedge_opened`).

### Montants à afficher (USDC, 7 décimales)

| Libellé | Formule | Source |
|---|---|---|
| Prix figé | `notional × locked_rate / 10¹⁴` | `quote.usdc_cost` ou `get_hedge` |
| Marge déposée | `margin` | `get_hedge` / `hedge_opened` |
| Protection jusqu'à | ± `cap_bps / 100` % | `get_hedge` |
| Gain / perte de la couverture | `pnl` (> 0 : le pool paie Lucía) | `hedge_settled` |
| Versé au règlement | `payout = margin + pnl` | retour de `settle` / `hedge_settled` |
| Coût des pesos | USDC débités par le path payment | résultat de la transaction |
| **Coût total réel** | `margin − payout + coût des pesos` | calcul front ; = prix figé tant que le mouvement reste dans le plafond |
| Sans protection | `notional × spot de fixing / 10¹⁴` | `hedge_settled.spot` |

### Erreurs à traduire

Les textes de `panic!` sont effacés dans le WASM déployé : le front ne reçoit qu'un **code** (`Error(Contract, #N)`).

| Code | Origine | Message à afficher |
|---|---|---|
| #200 `UnknownHedge` | Paiement | Couverture introuvable |
| #201 `AlreadySettled` | Paiement | Cette couverture est déjà réglée |
| #202 `NotMatured` | Paiement | L'échéance n'est pas encore atteinte (afficher le compte à rebours sur `maturity`) |
| #203 `InvalidNotional` | Paiement | Montant de facture invalide |
| #204 `InvalidDuration` | Paiement | Échéance invalide |
| #100 `InvalidAmount` | Vault | Montant invalide |
| #101 `InsufficientCapacity` | Vault | Le pool n'a pas assez de capital pour cette protection |
| #102 `ReleaseExceedsReserved` | Vault | Erreur interne (libération au-delà du capital réservé) |
| #103 `LossExceedsCapital` | Vault | La perte dépasse le capital du pool |
| #104 `PoolDepleted` | Vault | Le pool est vidé : dépôt refusé |
| #105 `DepositTooSmall` | Vault | Dépôt trop petit |
| #106 `InsufficientShares` | Vault | Pas assez de parts |
| #107 `WithdrawTooSmall` | Vault | Retrait trop petit, ou rien de libre à retirer pour l'instant |
| #108 `CurrencyLimitExceeded` | Vault | Limite atteinte sur cette devise |
| #109 `UnknownPosition` | Vault | Erreur interne (couverture non enregistrée dans le pool) |
| #110 `Overflow` | Vault | Montant trop grand |
| #111 `JuniorRatioTooLow` | Vault | La tranche dynamique doit rester ≥ 20 % du capital |
| #112 `InsufficientSeniorShares` | Vault | Pas autant de parts prudentes |
| #10 (contrat USDC / SAC) | Token USDC | Solde USDC insuffisant (marge de `open_hedge`, `deposit_lp`, `deposit_senior`) |
| #205 `InvalidTimeFactor` | Paiement (admin) | Facteur de démo invalide |
| #302 `StalePrice` | Pricing | Cours de l'oracle trop ancien : réessayer dans un instant |
| #306 `CurrencyNotEnabled` | Pricing | Devise non disponible |
| #307 `CapBelowMinimum` | Pricing | Protection minimale pour cette devise : x % |
| #308 `CapTooHigh` | Pricing | Protection trop élevée |
| #312 `NoFixing` | Pricing | Cours à l'échéance indisponible (règlement trop tardif) |
| #302 `StalePrice` au `settle` | Pricing | Aucun cours publié dans les 15 min avant l'échéance |
| autres 300-311 | Pricing | voir `PricingError` dans `pricing.rs` |
| `op_too_few_offers` | Path payment | Pas assez de liquidité sur le DEX pour ce montant |
| `op_over_source_max` | Path payment | Le cours a bougé au-delà de `send-max` : relancer avec un devis à jour |
| `op_underfunded` | Path payment | Solde USDC insuffisant pour payer le fournisseur |

### Événements

- `hedge_opened` — topics : `hedge_id`, `owner` ; data : `supplier`, `currency`, `side`, `notional`, `locked_rate`, `cap_bps`, `margin`, `maturity`
- `hedge_settled` — topics : `hedge_id`, `owner` ; data : `spot`, `pnl`, `payout`

Liens explorateur : `https://stellar.expert/explorer/testnet/tx/<hash>`.

---

## Chiffres de référence (1 000 000 MXN, 90 jours, spot 0,055, plafond 10 %, vrai Pricing)

Taux Etherfuse figés dans `scripts/rates.snapshot` (USD 3,60 %, MXN 5,55 %). Taux bloqué `F` = 0,0550128 (parité des taux sur 90 jours, puis spread 0,5 % contre Lucía) → prix figé **55 012,81 USDC**, marge **5 501,28 USDC**. Sur 60 s (sans mode démo) : 55 275 / 5 527,50.

| Peso | Spot | P&L | Versé par `settle` | Coût des pesos | Coût total | Sans protection |
|---|---|---|---|---|---|---|
| +8 % | 0,0594 | +4 387,19 | 9 888,47 | 59 400 | **55 012,81** | 59 400 |
| +12 % | 0,0616 | +5 501,28 (plafonné, écart 6 587) | 11 002,56 | 61 600 | **56 098,72** | 61 600 |
| −8 % | 0,0506 | −4 412,81 | 1 088,47 | 50 600 | **55 012,81** | 50 600 |

À +12 %, Lucía ne paie que ce qui dépasse son plafond (1 085,91 USDC au lieu de 6 587).
Avec un autre spot (mock aligné sur Reflector), recalculer avec `docs/video-script.md` § « Recalculer les chiffres ».

## Répétition du 25/10 (déploiement de Clément, +8 %)

| # | Transaction | Lien |
|---|---|---|
| 1 | `deposit_lp` 100 000 USDC | [c97f6e3d…](https://stellar.expert/explorer/testnet/tx/c97f6e3dbab45cce9f460bd688acc16d1e0c5b0faa0b1a9e0c77308760408f25) |
| 2 | `open_hedge` → id 0, marge 5 487 | [f9223c3d…](https://stellar.expert/explorer/testnet/tx/f9223c3dba5021aeb9e1d2ff78e34e809dd5eb717afcc92e1e7fc0729b24ea34) |
| 3 | `set_price` +8 % | [3894d338…](https://stellar.expert/explorer/testnet/tx/3894d3382e107b168991103277032e7f225f64b55fb8db987276d33d06cd5b2d) |
| 4 | `settle` (signé par le fournisseur) → 10 017 USDC | [091e9942…](https://stellar.expert/explorer/testnet/tx/091e994231c93ab45c45c6785be283c75284fabcb20f9881f1f5612f42b4b285) |
| 5 | Offre DEX réalignée à 0,0594 | [1b8ed23b…](https://stellar.expert/explorer/testnet/tx/1b8ed23bfbfb3ec0361346dcaf5e29654148e29565b15330975f03fb5e14d900) |
| 6 | Path payment : 1 000 000 MXN au fournisseur pour 59 400 USDC | [7a1913d8…](https://stellar.expert/explorer/testnet/tx/7a1913d8cff6b26bb9435e728859564b8866d9f9b8c2bc527cf4a64e72f3c488) |

Bilan Lucía : 200 000 → 145 130 USDC, soit **54 870 = le prix figé** (taux du stub de Pricing, 0,05487). Fait avant le branchement du vrai Vault et du vrai Pricing ; depuis, les tests Paiement passent sur le code fusionné.

## Répétition du 25/10 sur le code fusionné (vrai Vault + vrai Pricing, crash +12 %)

Déploiement de Clément refait avec `./scripts/setup-testnet.sh` de `main` (taux, plafonds minimum, dépôt LP de 1 000 000 USDC).

| # | Transaction | Résultat | Lien |
|---|---|---|---|
| 0 | `set_price` 0,055 (prix frais) | | [bf99fec1…](https://stellar.expert/explorer/testnet/tx/bf99fec1480132cd932a0798039ea30fb8d8aa05bcbfb1e9ceb4bfda4053f0d8) |
| — | `quote` (lecture) | taux 0,0552750, marge 5 527,50 | |
| 2 | `open_hedge` → id 0 | `position_reserved` + `hedge_opened`, même marge que `quote` | [3d068ca7…](https://stellar.expert/explorer/testnet/tx/3d068ca7e79f85a9b08fdb4344b7429ccb6e9dae3f6ca2fa7c46ca004cd004e6) |
| 3 | `set_price` +12 % | 0,0616 | [1cf406da…](https://stellar.expert/explorer/testnet/tx/1cf406daba881d15b77243e56239bdb31b363a13e4711ecee76de2f4bc73d2dd) |
| — | `settle` 20 s après l'ouverture | refusé `#202 NotMatured` (simulation, rien envoyé) | |
| 5 | Offre DEX réalignée (`616000:10000000`) | 0,0616 | [db3e392d…](https://stellar.expert/explorer/testnet/tx/db3e392d01bc1440dd9a72fd4cc6b4fe542896a7ae902706b36edffcd8c0373f) |
| 6 | Path payment 1 000 000 MXN | 61 600 USDC débités | [2fb310c3…](https://stellar.expert/explorer/testnet/tx/2fb310c3bc141cead47b977f1417a8cf8991764882fb5a9e8fa2baa83302fc0f) |
| 4 | `set_price` +12 % (rafraîchi) puis `settle` | `position_released` + `hedge_settled` : P&L plafonné 5 527,50, versé 11 055 | [0e7ff537…](https://stellar.expert/explorer/testnet/tx/0e7ff5373925826af2df7b069d9d7e213d7f7240a5a5056053c13b593d995507) |
| — | 2ᵉ `settle` / id 42 | `#201 AlreadySettled` / `#200 UnknownHedge` | |

Bilan Lucía : 545 130 → 489 057,50 USDC, soit **56 072,50** (le chiffre du script vidéo ; sans protection : 61 600).
Après `settle` : USDC du pool = `total_assets` = `capacity` = 994 472,50, exposition MXN à 0.

Note Windows : `setup-testnet.sh` appelle `python3`, qui sous Windows ouvre le Microsoft Store. Contournement : un alias `python3` → `python` dans le `PATH` le temps du script.

## Validation du fixing et du mode démo (25/10, code de `feat/payment` 9c37868)

**Fixing** (couverture à 120 s, sans mode démo) : crash +12 % posé 115 s avant l'échéance, puis peso à −8 % posé après l'échéance.
`lastprice` = 0,0506, `price(…, maturity)` = 0,0616 ; `settle` règle à **0,0616** et verse 2 × marge (11 054,99). Le prix posé après l'échéance est ignoré.
[crash](https://stellar.expert/explorer/testnet/tx/ca073de883d271aca4422662587f3fb14b16564f8e951cb6006a8be2d74a57f0) ·
[−8 % après l'échéance](https://stellar.expert/explorer/testnet/tx/48fb13e307b13a8a2210c6434754ab7e1b33c714f2be4433b9f79eb39ca8f334) ·
[settle](https://stellar.expert/explorer/testnet/tx/3546c5095230fafa4c42b16445f8c3e03917c98be3bfc914025b909a5060639c)

**Scénario de la vidéo** (mode démo 25 920, couverture à 90 jours, crash +12 %, `settle` sans rafraîchir le prix) :

| # | Transaction | Résultat | Lien |
|---|---|---|---|
| 0 | `set_time_factor 25920` | événement `time_factor_set` | [ebfa2300…](https://stellar.expert/explorer/testnet/tx/ebfa2300b85ab6863d3cf765fdc17233512c6f728db3645dc1f93c10ca3b6ff2) |
| — | `quote` 90 jours | taux 0,0548731, marge 5 487,31 | |
| 2 | `open_hedge` 7 776 000 s → id 1 | même taux et marge, échéance réelle +300 s | [b1f9fdbd…](https://stellar.expert/explorer/testnet/tx/b1f9fdbd71adcb4ecce4f35e28cf7e0adcb2cb82f8a65022e68d5efd9ea64192) |
| 3 | crash +12 % | avant l'échéance | [ffda98a4…](https://stellar.expert/explorer/testnet/tx/ffda98a465028fecaa8e896022118a8005c157f1c99172b2bee5f9b7d71d8c5a) |
| 5 | offre DEX à 0,0616 | | [9858c368…](https://stellar.expert/explorer/testnet/tx/9858c368ddff73a9a5735815f927534bd942bec637a5f1a514c55519bd91dade) |
| 6 | path payment 1 000 000 MXN | 61 600 USDC | [1361ff9b…](https://stellar.expert/explorer/testnet/tx/1361ff9b5626703364e53de0b4159ea904165b63d533e3a38a6319b7aefe6938) |
| 4 | `settle` 10 s après l'échéance | fixing 0,0616, versé 10 974,61 | [bc3f506f…](https://stellar.expert/explorer/testnet/tx/bc3f506f54060c2bf18d897e30ac2a1a87d5908ca713ad956a4f7487388b62ef) |

Coût total de Lucía : **56 112,69 USDC** au centime, le chiffre du script vidéo avec les taux de l'époque (USD 4 % / MXN 7 %). Avec les taux Etherfuse : 56 098,72.

