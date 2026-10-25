# <NOM>

> **Bloquez aujourd'hui le prix d'une facture fournisseur payable en devise dans 90 jours. Le jour J, votre fournisseur est payé dans sa monnaie, au taux promis.**

Projet HackMeridian (Stellar, Lisbonne, 25-26/10/2026), parcours **Genesis**, tracks **Stellar Unlocked** (path payments) + **Stellar Composed** (Reflector + Freighter).

---

## Le problème, en une histoire

Lucía importe de la tequila à Lisbonne. Elle doit **1 000 000 de pesos** à son fournisseur de Guadalajara **dans 90 jours**. Si le peso monte de 10 % d'ici là, sa facture lui coûte 5 500 $ de plus, et c'est sa marge qui disparaît.

Une grande entreprise appellerait sa banque pour bloquer le taux (un *forward de change*). Lucía n'a pas de ligne de crédit, et sa banque ne lui proposera jamais ce produit. Sur Stellar, elle sait payer à l'étranger en quelques secondes, mais **uniquement au comptant**. Rien ne lui permet de figer un taux pour un paiement futur.

## La solution

1. Lucía saisit sa facture : *1 000 000 MXN dans 90 jours*.
2. L'app lui affiche un prix figé : *« vous paierez 54 870 USDC, quoi qu'il arrive »*.
3. Elle choisit sa protection (par ex. ±10 %), dépose la marge correspondante en USDC et signe dans Freighter.
4. Le jour J, n'importe qui peut déclencher le règlement. Le contrat lit le cours sur l'oracle Reflector :
   - le peso a monté → le pool lui verse la différence ;
   - le peso a baissé → la différence est prise sur sa marge.
5. Dans la foulée, un **path payment Stellar** convertit ses USDC en pesos et paie le fournisseur : **1 000 000 MXN pile**, pour un coût total égal au prix figé au jour 0.

**Pourquoi Stellar :** la couverture et le paiement en devise du fournisseur passent par le même rail : USDC natif, oracle FX Reflector, path payment jusqu'au peso. Pas de banque, pas de ligne de crédit, et un règlement que n'importe qui peut déclencher.

---

## Comment ça marche

### Les acteurs

| Acteur | Rôle |
|---|---|
| **PME** (Lucía) | Fige le prix d'une facture future, dépose une marge |
| **LP** | Dépose des USDC dans le pool, sert de contrepartie, touche les spreads |
| **Contrat pool** (Soroban) | Détient les fonds, enregistre les couvertures, applique la règle de capacité |
| **Oracle Reflector** (SEP-40) | Cours de chaque devise contre USD |
| **DEX Stellar / anchor** | Convertit les USDC en devise locale pour payer le fournisseur |

### Le prix figé

Le prix bloqué n'est pas le cours du jour, mais le **taux forward** : le cours du jour ajusté de l'écart de taux d'intérêt entre les deux devises, plus le spread du pool.

```
Spot et F en USD par unité de devise (convention de l'oracle Reflector) :
F = Spot × (1 + r_USD × t) / (1 + r_devise × t)      prix proposé = F ± spread
```

Exemple : 1 MXN = 0,055 USD, r_MXN = 7 %, r_USD = 4 %, 90 jours → F ≈ 0,05460 USD. Le peso, mieux rémunéré, cote moins cher à terme. Sans cet ajustement, des arbitragistes pourraient gagner sans risque contre le pool.

### Une protection plafonnée, sans risque de défaut

La marge choisie par la PME fixe à la fois **sa perte maximale** et **sa protection maximale** :

| Marge déposée | Protégée si la devise bouge de |
|---|---|
| 5 % (minimum, plus élevé sur devises instables) | ±5 % |
| 10 % | ±10 % |
| 20 % | ±20 % |

Tout ce qui peut être perdu est **déjà déposé** dans le contrat : pas d'appel de marge, pas de liquidation, aucun risque de crédit. Techniquement, c'est un **tunnel de change** (collar) centré sur le taux forward : sa valeur est quasi nulle à l'ouverture, et le spread rémunère le pool.

> *« Mais la devise peut bouger de 15 % ? »* C'est la PME qui choisit son plafond. Et son alternative réelle n'est pas un forward bancaire gratuit, c'est **aucune couverture du tout** : la marge immobilisée est le prix d'une couverture sans banque.

### Le pool (vault)

**Que des dollars.** Le pool ne détient que des USDC. Chaque couverture est une jambe « devise contre dollar » : le pool n'a jamais besoin de stocker des pesos.

**Des parts, comme un fonds.** Un LP qui dépose des USDC reçoit des **parts**. Le premier dépôt se fait à 1 part = 1 USDC. Quand le pool gagne (le spread, ou une couverture qui tourne en sa faveur), chaque part vaut plus. Quand il perd, chaque part vaut moins. Personne n'a besoin de répartir les gains compte par compte.

**Deux prix pour une part :**

| Prix | Calcul | Sert à |
|---|---|---|
| prix plein | capital du pool ÷ nombre de parts | entrer (dépôt) |
| prix de sortie | (capital − capital réservé) ÷ nombre de parts | sortir (retrait) |

Le capital réservé est la perte maximale des couvertures en cours. Il n'est pas versé à celui qui sort. Personne ne peut donc partir en laissant ses pertes possibles aux autres. Les retraits sont équitables par construction : chaque LP ne peut retirer que sa part du capital libre. Si la couverture se termine bien, la décote laissée par celui qui est parti profite aux LPs restants.

**Règle de capacité : le pool ne peut pas faire faillite.** La perte maximale de chaque couverture (sa marge) est connue dès l'ouverture :

```
capital réservé = Σ marges des couvertures ouvertes   ≤   capital du pool
```

Le contrat **refuse toute couverture** qui ferait dépasser cette limite. Les marges des PME sont déposées dans le contrat mais n'appartiennent pas aux LPs. Les tests vérifient à chaque opération que `solde USDC du contrat = capital des LPs + marges des couvertures ouvertes`.

**Exposition et limite par devise.** Le pool suit, pour chaque devise, le volume des acheteurs et celui des vendeurs (en USDC au taux bloqué), ainsi que leurs marges. L'admin peut plafonner une devise, par exemple « les marges en ARS ne dépassent pas 10 % du capital ». Une limite à 0 suspend les nouvelles couvertures sur cette devise.

**Robustesse.** Les entrées de stockage sont prolongées automatiquement (120 jours). Les dépôts, retraits, ouvertures, clôtures et changements de limite émettent des événements lisibles sur l'explorateur. Tous les calculs sont protégés contre les débordements et arrondis en faveur du pool.

### Netting (calcul prêt, branchement en roadmap)

La contrepartie naturelle d'un importateur qui doit des pesos est un **exportateur mexicain** qui va en recevoir. Quand l'un gagne, l'autre perd : en théorie, seule l'exposition nette devrait consommer du capital.

Le contrat contient déjà le **calcul exact** de la perte maximale d'un panier de couvertures, sur tous les cours possibles, même avec des plafonds et des taux bloqués différents. Il est prouvé par des tests de propriété (variations de −30 % à +30 %, 50 paniers aléatoires, contre-exemples). Un « net des marges » naïf serait faux : avec des plafonds différents, ou des taux bloqués à des dates différentes, il sous-estime la perte.

Le MVP garde la règle brute, plus prudente. Le netting sera branché quand trois conditions seront réunies :
1. un fixing commun à l'échéance ;
2. des échéances arrondies à des dates de fixing ;
3. un règlement par panier, ou la comptabilisation de ce que doit la jambe opposée.

---

## Architecture

```
┌──────────────┐  Freighter  ┌──────────────────────────────────────┐
│ Front (Vite) │────────────▶│      Contrat pool (Soroban, Rust)    │
│  écran PME   │             │  deposit_lp / withdraw               │
│  panneau LP  │             │  quote / open_hedge / settle         │
└──────┬───────┘             │  capacity                            │
       │                     └──────────────┬───────────────────────┘
       │ path payment                       │ lastprice (SEP-40)
       ▼                                    ▼
┌──────────────┐             ┌──────────────────────────────────────┐
│ DEX Stellar  │             │ Reflector FX (spot réel affiché)     │
│ USDC → MXN   │             │ mock-oracle SEP-40 (démo : « crash ») │
└──────────────┘             └──────────────────────────────────────┘
```

Une transaction Soroban ne peut pas contenir d'opération classique : le front enchaîne `settle()` puis le path payment, en deux transactions.

### Interface du contrat

```rust
// Vault (montants et parts en 7 décimales)
fn deposit_lp(lp: Address, amount: i128) -> i128;        // USDC déposés → parts du LP
fn withdraw(lp: Address, amount: i128) -> i128;          // parts brûlées → USDC versés (prix de sortie)
fn lp_balance(lp: Address) -> i128;                      // valeur des parts au prix de sortie
fn lp_shares(lp: Address) -> i128;
fn share_price() -> i128;                                // prix plein, USDC par part
fn exit_price() -> i128;                                 // prix de sortie, USDC par part
fn total_assets() -> i128;                               // capital des LPs
fn capacity() -> i128;                                   // capital non réservé
fn exposure(currency: Symbol) -> Exposure;               // acheteurs / vendeurs / net, marges
fn currency_limit(currency: Symbol) -> Option<u32>;
fn set_currency_limit(currency: Symbol, limit_bps: u32); // admin
// Pricing
fn quote(currency: Symbol, side: Side, notional: i128, duration_secs: u64, cap_bps: u32) -> Quote;
fn set_rate(currency: Symbol, rate_bps: u32);            // admin
fn set_min_cap(currency: Symbol, cap_bps: u32);          // admin
fn set_spread(spread_bps: u32);                          // admin
// Couvertures / paiement
fn open_hedge(owner: Address, supplier: Address, currency: Symbol, side: Side,
              notional: i128, duration_secs: u64, cap_bps: u32) -> u64;
fn settle(hedge_id: u64) -> i128;                        // permissionless après l'échéance
fn get_hedge(hedge_id: u64) -> Option<Hedge>;
```

---

## Déploiement testnet

| Élément | ID |
|---|---|
| Contrat pool | `CAYRDFUI2G3FD7OZPDXOVXGVZYFEW2YOEUJPS7OPCQMNZKKOWMZYZCLH` |
| mock-oracle (SEP-40) | `CCCA5K5G7ZWVRTWXQ4MWYQQMAPPPBK5J6TUXTOIEYCF3HYDVOZGBQVTE` |
| Reflector FX (testnet) | `CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W` |
| USDC de test (SAC) | `CCLMFOUKC7ZQ5OPFE67F5XUGCOWTVCOOVTKTEPQGEYKWWRRKJGNVR2OC` |
| MXN de test (SAC) | `CAFK4PTVDK6Y33HWTSUPH57JV3G4YZFS6PL77FNGWOOMHBJJESYBMVMS` |

Reflector FX testnet couvre : EUR, GBP, CHF, CAD, **MXN, ARS, BRL**, THB, XAU (base USD, 14 décimales, résolution 300 s).

Le pool est redéployé à chaque `./scripts/setup-testnet.sh`, qui fait aussi le dépôt du LP (1 000 000 USDC via `deposit_lp`). Les IDs à jour sont dans `.env.testnet`.

## Lancer le projet

```sh
# contrats
stellar contract build
cargo test

# environnement testnet complet (comptes, actifs, offre DEX, contrats)
./scripts/setup-testnet.sh

# front
cd front && pnpm install && pnpm dev
```

## Structure

```
contracts/pool         # vault.rs (capital, parts LP, exposition) · pricing.rs · hedge.rs
contracts/mock-oracle  # oracle SEP-40 pilotable par l'admin (démo)
front/                 # Vite + React + TS, stellar-sdk, Stellar Wallets Kit
scripts/               # setup testnet
sim/                   # simulateur de pricing / risque (optionnel)
docs/council/          # council du 25/10 : rapport + transcript
TODO.md                # setup et chantiers
```

## Roadmap

**Juste après le hackathon**
- Paiement atomique : régler la couverture et payer le fournisseur dans une seule transaction (swap via un DEX Soroban)
- Marge productive : la marge de la PME placée sur Blend pendant la couverture
- Fixing robuste : TWAP, bornes de cohérence, plusieurs oracles

**Pool et risque**
- Netting importateurs / exportateurs par devise et par échéance
- Parts LP en token SEP-41, Blend sur le capital non réservé du pool
- Limites par devise, skew de prix, spread dynamique
- Taux d'intérêt lus onchain, fixing sur les cotations SEP-38 des anchors

**Produit et distribution**
- Facture couverte transférable : vendre une créance déjà couverte à un financeur (affacturage)
- SDK « Bloquer le taux » pour wallets et anchors
- KYC via les anchors (SEP-12) pour le cadre réglementaire
- Audit, puis mainnet

## Risques connus

| Risque | Parade |
|---|---|
| Défaut de la PME | Impossible : la perte max est déjà déposée |
| Faillite du pool | Impossible : règle de capacité |
| Flux tous dans le même sens (anti-sélection) | Limites par devise, skew de prix, spread dynamique |
| Taux d'intérêt codés en dur | Limite assumée du MVP, lecture onchain en roadmap |
| Oracle manipulé au fixing | TWAP sur une fenêtre, bornes de cohérence, plusieurs oracles |
| Écart oracle / cours réel du DEX ou de l'anchor | Résidu faible, affiché à l'utilisateur |

## Concurrence

*FX Swap by Hedgehog* (SCF #21) propose des swaps de devises avec livraison physique (USDC/EURC/AUDD) pour des utilisateurs DeFi. <NOM> vise les PME : une couverture sur n'importe quelle devise couverte par l'oracle, sans stock de devise dans le pool, reliée au paiement du fournisseur.
