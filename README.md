# <NOM>

> **Bloquez aujourd'hui le prix d'une facture fournisseur payable en devise dans 90 jours. Le jour J, votre fournisseur est payé dans sa monnaie, au taux promis.**

Projet HackMeridian (Stellar, Lisbonne, 25-26/10/2026), parcours **Genesis**, tracks **Stellar Unlocked** (path payments) + **Stellar Composed** (Reflector + Freighter).

---

## Le problème, en une histoire

Lucía importe de la tequila à Lisbonne. Elle doit **1 000 000 de pesos** à son fournisseur de Guadalajara **dans 90 jours**. Si le peso monte de 10 % d'ici là, sa facture lui coûte 5 500 $ de plus, et c'est sa marge qui disparaît.

Une grande entreprise appellerait sa banque pour bloquer le taux (un *forward de change*). Lucía n'a pas de ligne de crédit, et sa banque ne lui proposera jamais ce produit. Sur Stellar, elle sait payer à l'étranger en quelques secondes, mais **uniquement au comptant**. Rien ne lui permet de figer un taux pour un paiement futur.

## La solution

1. Lucía saisit sa facture : *1 000 000 MXN dans 90 jours*.
2. L'app lui affiche un prix figé : *« vous paierez 54 873 USDC, quoi qu'il arrive »*.
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

`r_USD` est le taux des bons du Trésor US tokenisés (~4 %) : c'est ce que rapporterait le capital du pool s'il ne servait pas à couvrir (voir *Où dort l'argent du pool*).

Le spot vient de **Reflector FX**, l'oracle de change natif de Stellar (SEP-40, mise à jour toutes les 300 s). Sur mainnet (`CBKGPWGKSKZF52CFHMTRR23TBWTPMRDIYZ4O2P5VS65BMHYH4DXMCJZC`), il couvre **24 devises** (vérifié onchain le 25/10/2026) : EUR, GBP, CAD, JPY, CNY, HKD, KRW, INR, PHP, TRY, RUB, ZAR, MXN, BRL, ARS, COP, CLP, PEN, CRC, VES, NGN, KES, CDF, XAU.

### Une protection plafonnée, sans risque de défaut

La marge choisie par la PME fixe à la fois **sa perte maximale** et **sa protection maximale** :

| Marge déposée | Protégée si la devise bouge de |
|---|---|
| 5 % (minimum, plus élevé sur devises instables) | ±5 % |
| 10 % | ±10 % |
| 20 % | ±20 % |

Tout ce qui peut être perdu est **déjà déposé** dans le contrat : pas d'appel de marge, pas de liquidation, aucun risque de crédit. Techniquement, c'est un **tunnel de change** (collar) centré sur le taux forward : sa valeur est quasi nulle à l'ouverture, et le spread rémunère le pool.

> *« Mais la devise peut bouger de 15 % ? »* C'est la PME qui choisit son plafond. Et son alternative réelle n'est pas un forward bancaire gratuit, c'est **aucune couverture du tout** : la marge immobilisée est le prix d'une couverture sans banque.

### Ce que paie la PME

Pour 1 000 000 MXN à 90 jours, plafond 10 %, spread 0,5 % (≈ 54 873 USDC) :

| Coût | Montant | % de la facture |
|---|---|---|
| Spread du pool | ~273 USDC | 0,50 % |
| Rendement perdu sur la marge bloquée (5 487 USDC à ~4 %) | ~54 USDC | 0,10 % |
| Écart forward / spot | — | pas un frais : c'est le juste prix (en faveur d'un importateur de pesos) |
| Frais Stellar, oracle | ~0 | ~0 |
| **Total** | **~330 USDC** | **~0,6 %** (≈ 2,4 %/an) |

La conversion USDC → MXN pour payer le fournisseur n'est pas comptée : elle est due avec ou sans couverture.

| Solution | Coût pour 90 jours | Accès pour une PME |
|---|---|---|
| Aucune couverture | 0, mais un mouvement de 6,5 % (fréquent sur 90 j) coûte ~3 600 $ | — |
| **<NOM>** | **~0,6 %** | sans banque ni ligne de crédit, marge de 5 à 20 % au choix |
| Forward bancaire | ~0,5 à 2 % | ligne de crédit, rarement accordée à une petite PME |
| Fintechs de change (Ebury, iBanFirst, Corpay) | ~0,5 à 1,5 % | dépôt initial de 5 à 10 %, KYC, devises limitées |
| Option d'achat (protection non plafonnée) | ~2,6 % (prime à la monnaie, vol 13 %) | rare pour une PME |

Environ **4 fois moins cher qu'une option**, parce que la protection est plafonnée dans les deux sens ; au prix d'un forward, mais sans banque. Contrepartie assumée : au-delà du plafond, la PME n'est plus protégée. *(Banques et fintechs : ordres de grandeur du marché, non vérifiés devise par devise.)*

### Le pool et sa règle de capacité

Les LPs déposent des USDC. La perte maximale du pool est connue à l'avance :

```
perte max = Σ (notionnel en USDC × plafond)   ≤   dépôts du pool
```

Le contrat **refuse toute couverture** qui ferait dépasser cette limite : le pool ne peut pas faire faillite par construction. Seul le capital non réservé est retirable.

### Les LPs : tranche senior et tranche junior (roadmap)

Les LPs déposent dans le même pool mais choisissent leur risque :

| | **Senior** (prudent) | **Junior** (risque) |
|---|---|---|
| Profil | Veut un rendement stable | Accepte de perdre pour gagner plus |
| Son argent sert à | Couvrir l'exposition (CETES, EURC) + trésorerie | Absorber l'exposition non couverte |
| Pertes | Protégé par le junior | Perd en premier |
| Rendement | Taux fixe payé en priorité (~5 %) | Tout le reste : spread, pertes des clients, trésorerie |

Règles du contrat :
1. **Perte max non couverte ≤ capital junior** : la règle de capacité s'applique au seul junior. Chaque contrat étant plafonné, le senior ne peut pas perdre à cause du change (il reste exposé aux protocoles tiers et à l'USDC).
2. **Cascade** : les pertes touchent le junior d'abord ; les gains paient d'abord le taux du senior, le junior prend le surplus.
3. **La part couverte est un réglage** : plus de junior, moins de couverture et plus de levier.
4. **Junior épuisé** : le pool n'accepte plus que des couvertures entièrement couvertes.

Simulation (pool de 1 M$ : 800 k$ senior, 200 k$ junior utilisé à 15 % ; peso à 13 % de vol avec sauts de 15 % ; spread 0,4 % par 90 j dont 20 % pour le protocole ; Blend 6,5 % plafonné à 20 %, bons du Trésor 4 %) :

| Netting | Senior | Junior (moyenne) | Années perdantes (junior) | Pire année sur 20 (junior) |
|---|---|---|---|---|
| non | 5,0 % | 7,7 % | 33 % | −21 % |
| oui (1 M$ compensé par trimestre) | 5,0 % | **16 %** | 18 % | −13 % |

Référence : la même trésorerie sans activité de couverture rapporte ~4,3 %. **Sans netting, le junior n'est pas assez payé pour son risque ; avec du netting, il le devient.** Utiliser plus le junior (30-50 %) monte le rendement mais rend une perte quasi totale possible : l'utilisation est plafonnée à 15-20 %.

### Où dort l'argent du pool (roadmap)

Quatre poches, dans l'ordre où le pool puise pour payer :

| Poche | Taille | Rendement | Rôle |
|---|---|---|---|
| USDC dans le contrat | 5 % des dépôts | 0 % | Règlements et retraits LP, sans dépendre d'un tiers |
| Blend | **20 % max** | 5,7 à 8,9 % (variable) | Rendement, sortie immédiate |
| Bons du Trésor US tokenisés (BENJI, Etherfuse USTRY… à vérifier) | le reste | ~4 % | Rendement stable |
| CETES / EURC | selon l'exposition nette | taux `r_USD` du pricing | La couverture |

Exemple, pool de 1 M$ + ~86 k$ de marges clients : 50 k$ en USDC, 200 k$ sur Blend, 276 k$ en bons du Trésor, 560 k$ en CETES.

Blend est plafonné parce que ~14 M$ seulement y sont retirables à un instant donné (78 % des dépôts USDC sont prêtés), et pour ne pas dépendre d'un seul protocole (exploit d'oracle du 22/02/2026). Une fonction `rebalance()` remet les poches à niveau : nouvelle couverture → achat de CETES ; échéance proche → retour vers la réserve d'USDC.

### Couverture du pool (roadmap)

Le plafond et la règle de capacité garantissent que le pool **ne fait pas faillite**, pas qu'il est rentable de façon régulière. Le pool ne parie pas sur la tendance (le taux forward l'intègre déjà), mais il reste exposé aux mouvements de la devise : environ 6,5 % en 90 jours sur le peso, contre un spread de 0,4 %. Sans couverture, il faudrait un spread de 3 à 5 % pour rémunérer les LPs, et aucune PME n'achèterait. Le pool se couvre donc en trois étages, comme une banque qui vend un forward.

```
Exposition au peso
 ├─ 1. Netting : les clients en sens inverse se compensent        → gratuit
 ├─ 2. Le reste net : le pool détient la devise (≈ 88 % du net),
 │      placée en obligations d'État tokenisées                  → quasi gratuit
 └─ 3. Ce qui n'est pas couvrable : limites par devise + règle de capacité
```

**1. Netting.** Lucía doit des pesos : elle craint que le peso **monte**. Sa contrepartie naturelle craint qu'il **baisse** : un **importateur mexicain** qui paie ses fournisseurs en dollars, ou un exportateur payé en pesos. Ce que le pool verse à l'un, il le reçoit de l'autre, et il garde les deux spreads. Seule l'exposition nette consomme du capital. Un **skew** de prix (le côté trop demandé paie plus cher, l'autre moins cher) attire le côté manquant. *Attention : un exportateur mexicain reçoit des dollars et craint lui aussi que le peso monte, il est du même côté que Lucía.*

**2. Détenir la devise.** Si le pool doit payer quand le peso monte, il achète des pesos maintenant : s'ils montent, ils compensent ce qu'il doit verser. Ces pesos sont placés au taux local, en **CETES tokenisées** (obligations d'État mexicaines émises sur Stellar par Etherfuse, à vérifier). Ce rendement local paie exactement l'écart entre le taux forward et le spot (parité des taux d'intérêt) : la couverture ne coûte que les frais de conversion, et le pool garde le spread.

| Devise | Couverture | Faisabilité |
|---|---|---|
| EUR | EURC natif, placé sur Blend | ✅ |
| MXN | CETES tokenisées (Etherfuse) | ✅ à vérifier (liquidité, accès) |
| BRL, ARS, NGN… | Stablecoins des anchors | ⚠️ selon la liquidité |
| Devises sans token | Teneur de marché partenaire, sinon limites serrées et plafond minimum plus haut | ⚠️ plus tard |

**3. Limiter le reste.** Plafond d'exposition nette par devise ; une devise n'est ouverte que si elle est couvrable ou si ses flux se compensent ; la règle de capacité reste le dernier filet.

**La couverture n'est pas totale.** Elle neutralise l'essentiel du risque de change. Le reste est petit, mesurable, et payé par le spread :
- **Protection plafonnée, couverture non plafonnée** : au-delà du plafond, la position en devise ne correspond plus (gain si le peso monte de 15 %, perte s'il baisse de 15 %). Il faut réduire la position quand le cours approche du plafond, et un saut brutal peut tomber entre deux ajustements.
- **Obligations** : prix sensible aux taux si on les revend avant leur terme (d'où des CETES à 28 jours), risque de l'émetteur du token, liquidité au moment du règlement.
- **Écarts de prix** : oracle au fixing vs prix d'exécution sur le DEX, taux codés en dur vs rendement réel des CETES, décalage entre l'ouverture du contrat et l'achat de la couverture.

**Couvrir le portefeuille, pas chaque contrat.** Acheter et revendre des CETES à chaque couverture coûterait ~2 % par an en conversions et mangerait tout le spread. Le pool tient donc **une seule position par devise** :
- la cible est calculée en continu (≈ 88 % de l'exposition nette sur toutes les couvertures ouvertes) ;
- le pool n'échange que l'écart avec la cible, et seulement au-delà d'un seuil (par ex. 10 %, ou une fois par jour) ;
- un contrat qui arrive à échéance remplacé par un nouveau, ou deux contrats en sens inverse, ne déclenchent **aucun échange** ;
- les CETES sont renouvelés à leur échéance sans repasser par l'USDC ;
- optimisation : à l'échéance, payer le fournisseur **directement en devise** depuis la couverture, ce qui économise deux conversions.

Avec la couverture, le spread ne paie plus le risque, seulement les coûts : voir *Ce que paie la PME*. Le rendement des LPs dépend surtout du **volume compensé** : voir *Tranche senior et tranche junior*.

Le MVP montre le filet (plafond + règle de capacité brute, plus prudente) ; netting et couverture automatique sont la prochaine étape.

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
contracts/pool         # vault.rs · pricing.rs · hedge.rs (interface figée ; pricing implémenté, vault et hedge en cours)
contracts/mock-oracle  # oracle SEP-40 pilotable par l'admin (démo)
front/                 # Vite + React + TS, stellar-sdk, Stellar Wallets Kit
scripts/               # setup testnet
sim/                   # simulateur de pricing / risque (optionnel)
docs/council/          # council du 25/10 : rapport + transcript
TODO.md                # setup et chantiers
```

## Roadmap

- **Netting** par devise et par échéance (PME qui paient en devise / PME qui en reçoivent)
- **Couverture automatique** de l'exposition nette : détention de la devise placée en obligations d'État tokenisées (CETES), EURC sur Blend ; une position par devise avec seuil de rééquilibrage
- **Tranches senior / junior** pour les LPs
- **Gestion de trésorerie** : USDC 5 %, Blend ≤ 20 %, bons du Trésor tokenisés pour le reste
- **Livraison directe en devise** au fournisseur depuis la couverture
- **Parts LP** en token SEP-41
- **Facture couverte transférable** : vendre une créance déjà couverte à un financeur (affacturage)
- **SDK « Bloquer le taux »** pour wallets et anchors
- Taux d'intérêt lus onchain, fixing sur TWAP et cotations SEP-38 des anchors
- Audit

## Risques connus

| Risque | Parade |
|---|---|
| Défaut de la PME | Impossible : la perte max est déjà déposée |
| Faillite du pool | Impossible : règle de capacité |
| LPs exposés aux mouvements de la devise | Netting, puis couverture de l'exposition nette en devise (voir *Couverture du pool*) |
| Flux tous dans le même sens (anti-sélection) | Limites par devise, skew de prix, spread dynamique |
| Saut de cours entre deux rééquilibrages | Limites par devise, tranche junior en première perte |
| LP qui retire avant un règlement perdant | Retraits différés à l'échéance suivante, ou prix des parts incluant les contrats ouverts |
| Protocoles tiers (Blend, émetteurs des CETES et bons du Trésor) | Plafond par protocole, réserve d'USDC dans le contrat |
| Taux d'intérêt codés en dur | Limite assumée du MVP, lecture onchain en roadmap |
| Oracle manipulé au fixing | TWAP sur une fenêtre, bornes de cohérence, plusieurs oracles |
| Écart oracle / cours réel du DEX ou de l'anchor | Résidu faible, affiché à l'utilisateur |

## Concurrence

*FX Swap by Hedgehog* (SCF #21) propose des swaps de devises avec livraison physique (USDC/EURC/AUDD) pour des utilisateurs DeFi. <NOM> vise les PME : une couverture sur n'importe quelle devise couverte par l'oracle, sans livraison physique obligatoire (le pool ne détient la devise que pour couvrir son exposition nette), reliée au paiement du fournisseur.
