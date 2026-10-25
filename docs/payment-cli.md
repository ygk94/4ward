# Parcours Paiement en ligne de commande

Le parcours de Lucía de bout en bout, sur le testnet : dépôt LP → `open_hedge` → crash → `settle` → réalignement DEX → path payment au fournisseur.
Validé le 25/10 sur le déploiement de Clément (liens en bas). Les commandes utilisent les **alias** (`fx-pool`, `fx-mock-oracle`, `fx-lucia`…) : elles se rejouent telles quelles sur n'importe quel déploiement fait par `./scripts/setup-testnet.sh`.

Conventions : montants en **stroops** (7 décimales : 1 USDC = `10000000`), prix de l'oracle en **14 décimales** (0,055 USD = `5500000000000`), plafond en **bps** (1000 = 10 %).

## 0. Préparation

```sh
cd fistea-hackmeridian
source .env.testnet          # ISSUER, USDC_SAC, POOL_ID, … du déploiement
```

**Prix du mock frais.** Dès que le vrai Pricing est branché, `forward_rate` (dans `open_hedge`) et `settle` lisent le spot, refusé s'il a plus de 300 s. Relancer `set_price` **juste avant `open_hedge` et juste avant `settle`** :

```sh
stellar contract invoke --id fx-mock-oracle --source-account fx-admin --network testnet -- \
  set_price --asset '{"Other":"MXN"}' --price 5500000000000          # 0,055 USD/MXN
```

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
  --notional 10000000000000 --duration_secs 60 --cap_bps 1000         # 1 000 000 MXN, 60 s, 10 %
```
Retour : l'id de la couverture (`0` pour la première). Signature : **Lucía seule**.
Effets : transfert USDC Lucía → pool de la marge, événement `hedge_opened`, `capacity` baisse de la marge, `exposure MXN` augmente.

## 3. Crash du peso (fx-admin)

```sh
stellar contract invoke --id fx-mock-oracle --source-account fx-admin --network testnet -- \
  set_price --asset '{"Other":"MXN"}' --price 6160000000000          # +12 % : 0,0616
```
(+8 % : `5940000000000`.) Aucun mouvement d'argent.

## 4. Règlement à l'échéance (n'importe quel compte)

Attendre `duration_secs` après l'ouverture (`get_hedge` → champ `maturity`, en secondes Unix), prix du mock frais (§ 0), puis :

```sh
stellar contract invoke --id fx-pool --source-account fx-supplier --network testnet -- \
  settle --hedge_id 0
```
Retour : l'USDC versé à Lucía (marge ± P&L, entre 0 et 2 × marge). Effets : transfert USDC pool → Lucía, événement `hedge_settled` (spot de fixing, P&L, versement).

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
| #101 `InsufficientCapacity` | Vault | Le pool n'a pas assez de capital pour cette protection |
| #108 `CurrencyLimitExceeded` | Vault | Limite atteinte sur cette devise |
| #109 `UnknownPosition` | Vault | Erreur interne (couverture non enregistrée dans le pool) |
| à venir | Pricing | Plafond trop bas pour cette devise / cours de l'oracle trop ancien |
| `op_too_few_offers` | Path payment | Pas assez de liquidité sur le DEX pour ce montant |
| `op_over_source_max` | Path payment | Le cours a bougé au-delà de `send-max` : relancer avec un devis à jour |
| `op_underfunded` | Path payment / `open_hedge` | Solde USDC insuffisant |

### Événements

- `hedge_opened` — topics : `hedge_id`, `owner` ; data : `supplier`, `currency`, `side`, `notional`, `locked_rate`, `cap_bps`, `margin`, `maturity`
- `hedge_settled` — topics : `hedge_id`, `owner` ; data : `spot`, `pnl`, `payout`

Liens explorateur : `https://stellar.expert/explorer/testnet/tx/<hash>`.

---

## Chiffres de référence (1 000 000 MXN, prix figé 0,05487, plafond 10 %)

| Peso | Spot | P&L | Versé par `settle` | Coût des pesos | Coût total | Sans protection |
|---|---|---|---|---|---|---|
| +8 % | 0,0594 | +4 530 | 10 017 | 59 400 | **54 870** | 59 400 |
| +12 % | 0,0616 | +5 487 (plafonné, écart 6 730) | 10 974 | 61 600 | **56 113** | 61 600 |
| −8 % | 0,0506 | −4 270 | 1 217 | 50 600 | **54 870** | 50 600 |

Marge déposée : 5 487 USDC. À +12 %, Lucía ne paie que ce qui dépasse son plafond (1 243 USDC au lieu de 6 730).
Le prix figé de 0,05487 est la valeur du stub de Pricing ; avec le vrai `forward_rate`, les chiffres bougent légèrement.

## Répétition du 25/10 (déploiement de Clément, +8 %)

| # | Transaction | Lien |
|---|---|---|
| 1 | `deposit_lp` 100 000 USDC | [c97f6e3d…](https://stellar.expert/explorer/testnet/tx/c97f6e3dbab45cce9f460bd688acc16d1e0c5b0faa0b1a9e0c77308760408f25) |
| 2 | `open_hedge` → id 0, marge 5 487 | [f9223c3d…](https://stellar.expert/explorer/testnet/tx/f9223c3dba5021aeb9e1d2ff78e34e809dd5eb717afcc92e1e7fc0729b24ea34) |
| 3 | `set_price` +8 % | [3894d338…](https://stellar.expert/explorer/testnet/tx/3894d3382e107b168991103277032e7f225f64b55fb8db987276d33d06cd5b2d) |
| 4 | `settle` (signé par le fournisseur) → 10 017 USDC | [091e9942…](https://stellar.expert/explorer/testnet/tx/091e994231c93ab45c45c6785be283c75284fabcb20f9881f1f5612f42b4b285) |
| 5 | Offre DEX réalignée à 0,0594 | [1b8ed23b…](https://stellar.expert/explorer/testnet/tx/1b8ed23bfbfb3ec0361346dcaf5e29654148e29565b15330975f03fb5e14d900) |
| 6 | Path payment : 1 000 000 MXN au fournisseur pour 59 400 USDC | [7a1913d8…](https://stellar.expert/explorer/testnet/tx/7a1913d8cff6b26bb9435e728859564b8866d9f9b8c2bc527cf4a64e72f3c488) |

Bilan Lucía : 200 000 → 145 130 USDC, soit **54 870 = le prix figé**. Fait avant le branchement du vrai Vault (stubs) : à rejouer après le merge de Pricing.
