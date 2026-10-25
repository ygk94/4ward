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

### Le pool et sa règle de capacité

Les LPs déposent des USDC. La perte maximale du pool est connue à l'avance :

```
perte max = Σ (notionnel en USDC × plafond)   ≤   dépôts du pool
```

Le contrat **refuse toute couverture** qui ferait dépasser cette limite : le pool ne peut pas faire faillite par construction. Seul le capital non réservé est retirable.

### Netting (roadmap)

La contrepartie naturelle d'un importateur qui doit des pesos est un **exportateur mexicain** qui va en recevoir. Quand l'un gagne, l'autre perd la même somme : le pool fait passer l'argent de l'un à l'autre et garde les deux spreads. Seule l'exposition nette consomme du capital. Le MVP applique la règle de capacité brute (plus prudente).

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
// Vault
fn deposit_lp(lp: Address, amount: i128) -> i128;
fn withdraw(lp: Address, amount: i128) -> i128;          // limité au capital non réservé
fn capacity() -> i128;
fn lp_balance(lp: Address) -> i128;
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
| Contrat pool | `CBEUIYUGVJFHRZORP4XCMPMLCISVLAAMK4OCDLILHP3HKGX2M3UB6CJR` |
| mock-oracle (SEP-40) | `CAJB3DAOV7HMIZL4L3DFDAH4LGLC7JOM64G24KSEBAJRPKS465PLE6T2` |
| Reflector FX (testnet) | `CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W` |
| USDC de test (SAC) | `CCLMFOUKC7ZQ5OPFE67F5XUGCOWTVCOOVTKTEPQGEYKWWRRKJGNVR2OC` |
| MXN de test (SAC) | `CAFK4PTVDK6Y33HWTSUPH57JV3G4YZFS6PL77FNGWOOMHBJJESYBMVMS` |

Reflector FX testnet couvre : EUR, GBP, CHF, CAD, **MXN, ARS, BRL**, THB, XAU (base USD, 14 décimales, résolution 300 s).

Le pool est redéployé à chaque `./scripts/setup-testnet.sh` : les IDs à jour sont dans `.env.testnet`.

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
contracts/pool         # vault.rs · pricing.rs · hedge.rs (interface figée, corps à écrire)
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
