# TODO Pricing — détaillé

> Équipe : **Lancelot = Pricing** (cette to-do), **Clément = Paiement** (`feat/payment`), **Hugo = Vault** + transversal. Le tableau « Qui fait quoi » de `TODO.md` (fichier d'Hugo) est à mettre à jour avec ces noms.
> Branche `feat/pricing`. Fichiers à nous : `contracts/pool/src/pricing.rs`, bloc Pricing de `contracts/pool/src/test.rs`, `front/src/pricing/`, `scripts/crash.sh`, `sim/`.
> On ne touche pas à `lib.rs`, `hedge.rs`, `vault.rs`, `shared/` : on demande.
> Deadline : **lundi 26/10 12h00**. Priorité absolue : `spot` + `forward_rate` + tests (bloque Paiement).

Légende : ⬜ à faire · 🔴 bloque quelqu'un d'autre · ✅ fait

---

## 0. Environnement (avant tout)

- ✅ `curl https://sh.rustup.rs -sSf | sh` puis `rustup target add wasm32v1-none` (Rust ≥ 1.84)
- ✅ `brew install stellar-cli` (≥ 28) · `brew install pnpm` (ou `corepack enable`)
- ✅ `cargo test` à la racine : tout doit compiler (10 tests `#[ignore]`)
- ✅ `stellar contract build` : wasm de `pool` et `mock_oracle` dans `target/wasm32v1-none/release/`
- ✅ Récupérer `front/.env.local` auprès d'Hugo (ou `./scripts/setup-testnet.sh`), `cd front && pnpm install && pnpm dev`
- ✅ Freighter en Testnet, importer `fx-admin` (nécessaire pour le bouton crash) et `fx-lucia`

---

## 1. Contrat `pricing.rs`

### 1.1 ✅ `spot(env, currency) -> i128`
Doit marcher **à l'identique** sur le mock-oracle et sur Reflector FX.
- ✅ Déclarer un client SEP-40 minimal via `soroban_sdk::contractclient` :
  ```rust
  #[contracttype] pub enum Asset { Stellar(Address), Other(Symbol) }
  #[contracttype] pub struct PriceData { pub price: i128, pub timestamp: u64 }
  #[contractclient(name = "OracleClient")]
  pub trait Sep40 { fn lastprice(env: Env, asset: Asset) -> Option<PriceData>; fn decimals(env: Env) -> u32; }
  ```
  Les types doivent avoir exactement les mêmes noms/variantes que ceux du mock (`Asset::Other(Symbol)`, `PriceData{price,timestamp}`) pour que l'encodage XDR coïncide.
- ✅ Lire `DataKey::Oracle` dans l'instance, appeler `lastprice(Asset::Other(currency))`
- ✅ `None` → `panic!("no price")` ; `price <= 0` → panic
- ✅ Fraîcheur : `env.ledger().timestamp() - timestamp > MAX_AGE` → `panic!("stale price")`.
  Reflector met à jour toutes les 300 s → `MAX_AGE = 900` (3 résolutions) laisse de la marge. Attention : le mock horodate au `set_price`, donc en test il faut avancer le ledger avec `env.ledger().set_timestamp(...)` **avant** `set_price`, ou re-setter le prix.
- ✅ (option) vérifier `decimals() == 14` une fois, ou documenter l'hypothèse. Reflector FX = 14, mock = 14.

### 1.2 ✅ `forward_rate(env, currency, side, duration_secs) -> i128`
```
F = S × (1 + r_USD·t) / (1 + r_dev·t)      t = duration_secs / 31_536_000
```
- ✅ Tout en `i128`, en bps. Avec `SECS_PER_YEAR = 31_536_000` et `BPS = 10_000` :
  ```
  num = BPS × SECS_PER_YEAR + r_usd_bps × duration
  den = BPS × SECS_PER_YEAR + r_dev_bps × duration
  F   = S × num / den
  ```
  Ordre des opérations : multiplier **avant** de diviser (S ≈ 5,5e12, num ≈ 3,2e11 → 1,8e24, largement sous i128::MAX ≈ 1,7e38 ; vérifier avec un spot BRL/EUR ≈ 1e14 et une durée d'1 an : toujours OK).
- ✅ Taux lus dans `DataKey::Rate(currency)` et `DataKey::Rate("USD")` ; absent → panic `"rate not set"` (pas de défaut silencieux : un taux manquant = prix faux = arbitrage).
- ✅ Spread **contre l'utilisateur**, lu dans `DataKey::SpreadBps` :
  - `BuyCurrency` (Lucía paie la devise) → `F × (BPS + spread) / BPS`
  - `SellCurrency` → `F × (BPS − spread) / BPS`
- ✅ Arrondis en faveur du pool : `BuyCurrency` → arrondir **vers le haut** (ceil : `(a + b − 1) / b`), `SellCurrency` → arrondir vers le bas (division entière). Factoriser un helper `mul_div_ceil` / `mul_div_floor`.
- ✅ `duration_secs == 0` → F = S (autorisé pour la démo en secondes). Durée > 1 an → panic (hors produit).

### 1.3 ✅ `quote(...) -> Quote`
- ✅ `spot`, `forward_rate`, `usdc_cost = to_usdc(notional, forward_rate)`, `margin = usdc_cost × cap_bps / BPS` (ceil)
- ✅ Appeler `check_cap` dedans pour que le front ait l'erreur avant `open_hedge`
- ✅ `notional <= 0` → panic
- ✅ Vérifier que `to_usdc` est cohérent : 10_000_000_000_000 (1 M MXN, 7 déc.) × 5_500_000_000_000 (0,055, 14 déc.) / 1e14 = 550_000_000_000 = 55 000 USDC ✓. Proposer à Hugo de passer `to_usdc` en ceil (c'est son fichier ? non, il est dans `pricing.rs` : à nous).

### 1.4 ✅ `check_cap` / `set_min_cap`
- ✅ `set_min_cap` : `admin(env).require_auth()`, `cap_bps ≤ BPS`, stocker `DataKey::MinCap(currency)` en instance
- ✅ `check_cap` : `cap_bps < MinCap(currency)` → panic `"cap below minimum"` ; devise sans minimum → panic (devise non activée = refusée, c'est la liste blanche des devises) ; `cap_bps > 5000` → panic (plafond absurde)
- ✅ Valeurs : MXN 500 (5 %), BRL 700 (7 %), ARS 1500 (15 %), EUR 300 (3 %) — à calibrer avec `sim/` (§4)

### 1.5 ✅ `set_rate` / `set_spread`
- ✅ `admin(env).require_auth()` dans les deux
- ✅ `set_rate(currency, bps)` : `bps ≤ 10_000` (100 %/an, ARS peut être haut), stocker `Rate(currency)`
- ✅ `set_spread(bps)` : `bps ≤ 500`
- ✅ Valeurs sourcées (noter la source dans un commentaire + dans le deck) :
  | Devise | Taux directeur ~oct. 2026 | bps |
  |---|---|---|
  | USD | Fed funds ≈ 4 % | 400 |
  | MXN | Banxico ≈ 7 % | 700 |
  | BRL | Selic ≈ 10,5 % | 1050 |
  | ARS | Pas de taux directeur depuis la fin des LEFI (pilotage par agrégats) ; proxy marché : plazo fijo 30 j 16–24 % (Banco Nación 19,25 %, La Nación 21/10/2026), BADLAR ≈ 22 % (ARCA, 20/09/2026) | 2200 |
  | EUR | BCE ≈ 2 % | 200 |
  → vérifier les chiffres réels avant la vidéo, les mettre dans `scripts/setup-testnet.sh` via Hugo (§5).

### 1.6 ✅ `extend_ttl`
- ✅ Tout est en storage `instance` (Rate, MinCap, Spread) : `env.storage().instance().extend_ttl(...)` dans `quote` suffit (ou laisser à Vault qui le fait sur l'instance). À demander à Hugo pour éviter le doublon.

### 1.7 ✅ Codes d'erreur (`PricingError`, 300+)
Convention d'équipe (`TODO.md`) : Vault 100-199 · Paiement 200-299 · Pricing 300-399. Les textes de `panic!` sont effacés dans le WASM (on l'a vu sur le testnet : `UnreachableCodeReached` sans message) ; le front ne peut traduire qu'un code.

| Code | Variante | Quand | Message proposé pour le front |
|---|---|---|---|
| 300 | `NoPrice` | l'oracle n'a pas de prix pour la devise | « Cours indisponible pour cette devise » |
| 301 | `InvalidPrice` | prix ≤ 0 | « Cours invalide, réessayez plus tard » |
| 302 | `StalePrice` | prix de plus de 900 s | « Cours trop ancien, rafraîchissez le cours » |
| 303 | `RateNotSet` | taux de la devise ou USD absent | « Devise pas encore configurée » |
| 304 | `DurationTooLong` | durée > 1 an | « Échéance maximale : 1 an » |
| 305 | `InvalidNotional` | montant ≤ 0 | « Montant invalide » |
| 306 | `CurrencyNotEnabled` | pas de `set_min_cap` (liste blanche) | « Devise non disponible » |
| 307 | `CapBelowMinimum` | plafond sous le minimum de la devise | « Protection minimale pour cette devise : X % » |
| 308 | `CapTooHigh` | plafond > 50 % | « Protection maximale : 50 % » |
| 309 | `RateTooHigh` | admin : `set_rate` > 100 %/an | (admin) |
| 310 | `CapOutOfRange` | admin : `set_min_cap` à 0 ou > 50 % | (admin) |
| 311 | `SpreadTooHigh` | admin : `set_spread` > 5 % | (admin) |

Tests : chaque refus est vérifié avec son code exact (`Err(Ok(err(PricingError::…)))` ou `Error(Contract, #3xx)`), y compris la propagation par `quote` (`pricing_quote_propagates_oracle_and_rate_errors`). 22 tests Pricing verts.

---

## 1bis. Retours de Clément (Paiement, commit 9d215de sur `feat/payment`) — à prendre en compte

Ce que `hedge.rs` attend de nous est conforme à ce qu'on a livré : `spot`, `forward_rate` (formule du README, pas celle inversée du transcript du council), `check_cap`, `to_usdc` inchangée. Points d'action :

- ⬜ 🔴 **Fraîcheur du prix pendant la démo** : `settle` appelle `spot`, qui refuse un prix de plus de `MAX_PRICE_AGE_SECS` = 900 s (Clément pense 300 s : lui préciser). Le mock horodate au `set_price`. Conséquence pour la vidéo : le crash (ou un simple « rafraîchir le cours ») doit avoir lieu **moins de 15 min avant `settle`**. À prévoir dans `CrashButton` (bouton « Rafraîchir le cours ») et dans `scripts/crash.sh` (`--refresh` : repose le même prix pour mettre à jour l'horodatage). Le dire à Clément pour son script vidéo.
- ⬜ **Marge** : Clément arrondit déjà vers le haut (`to_usdc × cap_bps / BPS`, ceil), donc marge prélevée = marge affichée par `quote`. Lui proposer quand même d'appeler `pricing::margin_for(usdc_cost, cap_bps)` pour n'avoir qu'une seule formule.
- ⬜ **Durée nulle** : `open` refuse `duration_secs == 0`, `quote` l'accepte. Dans `QuoteCard`, imposer une durée ≥ 1 s pour ne pas afficher un devis que `open_hedge` refusera.
- ⬜ **Tests après merge** : Clément ajoute `mock-oracle` en dev-dependency de `contracts/pool/Cargo.toml`. Une fois mergé, on pourra remplacer notre `TestOracle` par le vrai `mock_oracle::MockOracle` dans `pricing_tests` (optionnel, gain de fidélité). Conflit attendu sur `test.rs` : nul si chacun reste dans son bloc, nos imports sont dans le module `pricing_tests`.
- ⬜ **Après merge**, les tests `open_*` de Clément prendront le vrai `forward_rate` : lui donner les valeurs exactes (spot 0,055, 400/700 bps, 90 j, spread 50 → Buy 5_487_305_413_412 ; marge 10 % sur 1 M MXN = 54_873_054_135).
- ✅ **Setup testnet** : sans `set_min_cap`, **aucune devise n'est cotable** (liste blanche). Bloc shell + message pour Hugo rédigés au §5 (25/10).
- ⬜ Point de vive voix avec Hugo (relayé par Clément) : ses clés de stockage Vault et l'API d'exposition par devise. Côté Pricing, on n'utilise que `Oracle`, `SpreadBps`, `Rate`, `MinCap` de `DataKey` : pas d'impact tant que `lib.rs` ne bouge pas.

---

## 2. Tests (bloc Pricing de `test.rs`)

Mise en place commune (helper dans le bloc Pricing, ne pas toucher aux autres blocs) :
- ✅ (fait avec notre `TestOracle` dans `pricing_tests`, en attendant la dev-dependency `mock-oracle` de Clément, cf. §1bis) `env.mock_all_auths()`, enregistrer le mock-oracle **par son wasm** (`contractimport!(file = "../../target/wasm32v1-none/release/mock_oracle.wasm")`) ou par crate `mock-oracle` en dev-dependency (plus simple : `[dev-dependencies] mock-oracle = { path = "../mock-oracle" }` puis `env.register(mock_oracle::MockOracle, ...)` — demander à Hugo pour le `Cargo.toml` de `pool`, c'est un fichier partagé). Préférer le `contractimport!` car la CI fait `cargo test` sans `stellar contract build`… donc **la dev-dependency est la bonne option** pour la CI.
- ✅ `set_price(MXN, 5_500_000_000_000)`, `set_rate(USD, 400)`, `set_rate(MXN, 700)`, `set_spread(50)`, `set_min_cap(MXN, 500)`

Tests à retirer de `#[ignore]` :
- ✅ `pricing_forward_parity` : spot 0,055, 90 j (7_776_000 s), spread 0 → F = 5_500_000_000_000 × (10_000×31_536_000 + 400×7_776_000) / (10_000×31_536_000 + 700×7_776_000). Calculer la valeur exacte attendue en i128 (≈ 5_459_7xx_xxx_xxx, soit ≈ 0,05460) et l'assert à l'unité près (pas de « ≈ »).
- ✅ `pricing_spread_direction` : avec spread 50 bps, `Buy ≥ F_sans_spread`, `Sell ≤ F_sans_spread`, et `Buy − F ≈ F × 0,5 %` ; vérifier aussi que ceil/floor ne se croisent jamais (`Sell < Buy`).
- ✅ `pricing_min_cap` : `check_cap(MXN, 400)` panique (`#[should_panic]`), `500` passe, devise inconnue panique.
- ✅ Nouveaux tests (ajouter au bloc) :
  - `pricing_quote_amounts` : 1 M MXN, cap 10 % → `usdc_cost` ≈ 54 870 USDC avec spread, `margin` = 10 % de ça, cohérent avec le README (« vous paierez 54 870 USDC »). Recalculer le chiffre exact et **mettre à jour le README si ça diffère** (demander à Hugo).
  - `pricing_stale_price_panics` : `set_price`, puis `env.ledger().set_timestamp(now + 901)`, `spot` panique.
  - `pricing_no_price_panics` : devise sans prix → panic.
  - `pricing_rate_missing_panics` : `forward_rate` sans `set_rate` → panic.
  - `pricing_set_rate_requires_admin` : sans `mock_all_auths` → échec d'auth.
  - `pricing_duration_zero_is_spot` : F(0 s) = S ± spread.
- ✅ `cargo test -p pool` vert (21 tests Pricing)
- ⬜ CI verte sur le push (rien n'est encore poussé)

---

## 3. Front `front/src/pricing/`

Dépend de `shared/wallet.ts` et `shared/contract.ts` (Hugo, 🔴). En attendant : appeler le contrat en **simulation** directement avec `@stellar/stellar-sdk` (`rpc.Server.simulateTransaction`), ce qui ne requiert pas de wallet pour `quote` et `lastprice`.

### 3.1 `QuoteCard.tsx` (utilisé par `payment/HedgeScreen.tsx`)
- ⬜ Définir les props avec Paiement et les figer tôt :
  ```ts
  type QuoteCardProps = { currency: 'MXN' | 'BRL' | 'ARS'; notional: string; durationSecs: number; capBps: number; side?: 'BuyCurrency' | 'SellCurrency'; onQuote?: (q: Quote) => void }
  ```
- ⬜ `pricing/oracle.ts` : `readSpot(contractId, currency)` → simulation de `lastprice({Other: currency})` sur **Reflector** (`config.reflectorFxId`) ET sur le mock (`config.mockOracleId`). Affichage : « Cours réel (Reflector) : 1 MXN = 0,0550 $ » + « Cours de démo (oracle de test) » quand ils divergent.
- ⬜ `pricing/quote.ts` : simulation de `pool.quote(currency, side, notional, duration, cap)` → `Quote { spot, forward_rate, usdc_cost, margin }` ; décodage des `i128` (14 et 7 déc.) en helpers `fmtPrice14`, `fmtAmount7`.
- ⬜ Rendu, zéro jargon : « Prix figé : vous paierez **54 873 USDC** quoi qu'il arrive » · « Protection ±10 % » · « Dépôt de garantie : 5 487 USDC (restitué à l'échéance, ± l'écart) » · petit texte « taux à terme = cours du jour ajusté de l'écart de taux MXN 7 % / USD 4 % ».
- ⬜ États : chargement, erreur (`cap below minimum` → message « protection minimale pour le peso : 5 % »), rafraîchissement quand les props changent (debounce 300 ms).
- ⬜ Exposer `onQuote` pour que Paiement récupère `margin` (montant à approuver dans Freighter).

### 3.2 `CrashButton.tsx`
- ⬜ Lire le prix MXN courant du mock, calculer `× 1,12` (le peso **monte** de 12 % → le prix USD/MXN monte), construire la tx `set_price({Other:'MXN'}, newPrice)` sur `config.mockOracleId`, signer avec Freighter (compte `fx-admin` sélectionné), envoyer, attendre la confirmation.
- ⬜ Afficher avant/après : « Peso : 0,0550 $ → 0,0616 $ (+12 %) » et le lien stellar.expert de la tx.
- ⬜ Bouton secondaire « Remettre le cours réel » : relit Reflector et le pousse dans le mock (= tâche « aligner le mock sur le spot » en un clic).
- ⬜ Désactivé si le wallet connecté n'est pas l'admin (afficher pourquoi).
- ⬜ Prévenir Paiement quand le prix change : l'offre DEX MXN/USDC est à 0,055 ; après le crash le path payment passera quand même (c'est l'offre d'Hugo qui fixe le cours DEX), mais le « coût total réel » affiché doit en tenir compte.

### 3.3 Qualité
- ⬜ `pnpm build` et `pnpm lint` verts (la CI build le front)
- ⬜ Pas de `any` ; `i128` manipulés en `bigint`

---

## 4. `scripts/crash.sh` + `sim/`

### 4.1 `scripts/crash.sh` (filet de sécurité vidéo)
- ⬜ `source .env.testnet`, argument `PCT` (défaut 12), devise (défaut MXN) :
  ```sh
  CUR=$(stellar contract invoke --id $MOCK_ORACLE_ID --network testnet --source-account fx-admin -- lastprice --asset '{"Other":"MXN"}' | jq -r .price)
  NEW=$(( CUR * (100 + PCT) / 100 ))
  stellar contract invoke --id $MOCK_ORACLE_ID --network testnet --source-account fx-admin -- set_price --asset '{"Other":"MXN"}' --price $NEW
  ```
- ⬜ Option `--reset` : lit Reflector (`lastprice` sur `$REFLECTOR_FX_ID`) et le pousse dans le mock.
- ⬜ `chmod +x`, tester une fois sur testnet.

### 4.2 `sim/` (chiffres pour le deck)
- ⬜ `sim/pnl.py` (ou notebook) : P&L de Lucía pour un mouvement du peso de −20 % à +20 %, **sans** protection, **avec** protection 5 / 10 / 20 %. Courbe « coût total en USDC » : plate entre ±cap, pente 1 au-delà. Export PNG pour le deck.
- ⬜ Reproduire les 3 chiffres de la démo : +12 % sans protection ≈ +6 600 $, avec protection 10 % ≈ +1 100 $ (les 2 % au-delà du plafond). Vérifier avec les mêmes formules i128 que le contrat (porter `forward_rate` en Python entier pour contrôle croisé).
- ⬜ Calibration spread / plafond minimum : volatilité annualisée approximative (MXN ~12 %, BRL ~15 %, ARS >> ), proba de dépasser le cap en 90 j (normale, σ√t), espérance de perte du pool au-delà du cap → justifier MXN 5 %, BRL 7 %, ARS 15 % et le spread 50 bps. Un tableau suffit.

---

## 5. Livrables aux autres

- ✅ 🔴 À Hugo, pour `scripts/setup-testnet.sh` : bloc rédigé et testé (25/10), message ci-dessous **à envoyer**
- ⬜ 🔴 Appliqué par Hugo sur le testnet (redéploiement du pool + bloc), puis `quote` vérifié contre les valeurs de référence

**État vérifié le 25/10 vers 19h40 (lecture seule, `--send=no`) :**
- Le pool déployé (`CBEUIYUG…6CJR`) date du squelette : `set_rate` et `quote` → `UnreachableCodeReached` (`panic!("TODO")`). **Il faut le redéployer** avec le wasm de `feat/pricing` (ou du merge) avant le bloc ci-dessous.
- `fx-admin` n'existe pas sur la machine de Lancelot : seul Hugo peut redéployer / appeler les fonctions admin.
- Syntaxe validée : les 9 commandes ci-dessous passent le parseur du CLI contre la spec du pool déployé et arrivent au contrat avec les bons arguments (`data:[USD, 400]` …). ⚠️ Le `set -- $r` suppose bash (le script a `#!/usr/bin/env bash`) ; sous zsh le découpage ne se fait pas.
- Prix du mock **périmés** (MXN posé il y a ~2 h) : tout `quote` / `settle` sur le mock échoue `stale price` (900 s). Il faut reposer les prix moins de 15 min avant chaque devis/règlement (→ `crash.sh --refresh`, §4.1).
- Le mock n'a **pas de prix EUR** : ouvrir EUR sans `set_price EUR` → `no price`.
- Reflector au même instant : MXN 0,05325 · BRL 0,19816 · ARS 0,000654 · EUR 1,1193 (mock : 0,055 · 0,18 · 0,001 · —).

**Message pour Hugo (à copier) :**

> Salut Hugo, c'est Lancelot (Pricing). Pour `scripts/setup-testnet.sh`, deux changements, bloquants pour le parcours de Clément :
>
> 1. Dans la boucle `set_price` du mock, ajouter l'EUR (valeur Reflector du 25/10) :
>    `for p in "MXN 5500000000000" "BRL 18000000000000" "ARS 100000000000" "EUR 111930000000000"; do`
>
> 2. Juste après `echo "   pool = $POOL"`, insérer :
> ```sh
> say "Paramètres Pricing (taux d'intérêt, plafonds minimum)"
> # Taux annuels en bps. Sources (oct. 2026) : Fed ≈ 4 %, Banxico ≈ 7 %, Selic ≈ 10,5 %, BCE ≈ 2 %,
> # ARS : pas de taux directeur BCRA, proxy BADLAR / plazo fijo ≈ 22 %. Détail : docs/todo-pricing.md §1.5
> for r in "USD 400" "MXN 700" "BRL 1050" "ARS 2200" "EUR 200"; do
>   set -- $r
>   stellar contract invoke --id $POOL --source-account $ADMIN --network $NET --quiet -- \
>     set_rate --currency $1 --rate_bps $2 >/dev/null
> done
> # Plafond minimum par devise. LISTE BLANCHE : une devise sans set_min_cap n'est PAS cotable
> # (quote et open_hedge paniquent « currency not enabled »).
> for c in "MXN 500" "BRL 700" "ARS 1500" "EUR 300"; do
>   set -- $c
>   stellar contract invoke --id $POOL --source-account $ADMIN --network $NET --quiet -- \
>     set_min_cap --currency $1 --cap_bps $2 >/dev/null
> done
> ```
> Le spread (50 bps) est déjà posé par le constructeur, rien à ajouter.
>
> ⚠️ Le pool actuellement déployé contient encore les `panic!("TODO")` : `set_rate` y échoue (`UnreachableCodeReached`). Il faut redéployer avec le wasm de `feat/pricing` (ou après le merge de lundi 8h) avant que ce bloc passe.
>
> Pour vérifier après coup (prix MXN du mock reposé depuis moins de 15 min, sinon `stale price`) :
> ```sh
> stellar contract invoke --id $POOL --source-account fx-admin --network testnet --send=no -- \
>   quote --currency MXN --side BuyCurrency --notional 10000000000000 --duration_secs 7776000 --cap_bps 1000
> ```
> Attendu : `spot` 5500000000000, `forward_rate` 5487305413412, `usdc_cost` 548730541341, `margin` 54873054135.
> Au passage, le README dit « 54 870 USDC » : le chiffre exact est **54 873 USDC** (54 873,05), à corriger.

- ⬜ À Hugo (front) : table des codes 300-311 du §1.7 pour les messages d'erreur de `QuoteCard` / écran Paiement. Message à copier :
  > Hugo, côté Pricing les refus sont maintenant des codes `PricingError` 300-311 (table + messages proposés dans `docs/todo-pricing.md` §1.7). Le client TS les reçoit comme `Error(Contract, #3xx)`. Les plus probables en démo : 302 (cours trop ancien → rafraîchir le mock), 307 (protection sous le minimum, MXN 5 %), 306 (devise non ouverte). Au passage : `spot` accepte un prix jusqu'à **900 s**, pas 300 s comme écrit dans `TODO.md`.
- ⬜ À Hugo : dev-dependency `mock-oracle` dans `contracts/pool/Cargo.toml` (ou accord pour que je l'ajoute)
- ⬜ À Paiement : props de `<QuoteCard />` figées + `onQuote` ; signaler tout changement du prix mock (offre DEX)
- ✅ README corrigé : 54 873 USDC (25/10 soir)
- ⬜ À Hugo : les slides pricing (§6)

---

## 6. Pitch (slides Pricing, à donner à Hugo dimanche soir)

- ⬜ Slide « Le prix figé est un taux à terme, pas le cours du jour » : formule, exemple MXN (0,0550 → 0,0546), « sans ça, un arbitragiste gagne sans risque contre le pool ».
- ⬜ Slide « Un tunnel de change à perte max connue » : schéma collar centré sur F, zone plate ±cap, « valeur ≈ 0 à l'ouverture, le spread rémunère le pool », courbe `sim/`.
- ⬜ Réponses aux objections (1 phrase chacune, notes du pitch) :
  - plafond vs mouvements de 15 % → la PME choisit, minimum par devise, alternative réelle = aucune couverture
  - marge immobilisée vs banque → pas de ligne de crédit, pas d'appel de marge, pas de défaut
  - anti-sélection → limites par devise, skew en roadmap
  - taux en dur → limite assumée, lecture onchain en roadmap, valeurs sourcées
  - fixing manipulable → TWAP / bornes en roadmap, oracle Reflector multi-sources
  - réglementation des dérivés → MVP testnet, KYC via anchors en roadmap

---

## 7. Suites de la discussion « économie du pool » (25/10 soir)

Le README a gagné les sections *Ce que paie la PME*, *Tranche senior et tranche junior*, *Où dort l'argent du pool* et *Couvrir le portefeuille, pas chaque contrat* (non commitées, sur `feat/pricing`). Conséquences pour Pricing :

- ✅ Oracle : Reflector FX reste le choix (seul oracle de change SEP-40 sur Stellar). Mainnet `CBKGPWGKSKZF52CFHMTRR23TBWTPMRDIYZ4O2P5VS65BMHYH4DXMCJZC` = **24 devises** vérifiées onchain (EUR, GBP, CAD, JPY, CNY, HKD, KRW, INR, PHP, TRY, RUB, ZAR, MXN, BRL, ARS, COP, CLP, PEN, CRC, VES, NGN, KES, CDF, XAU), testnet = 9. Pyth = 2ᵉ source possible en roadmap (pull, non SEP-40) ; Chainlink, Band, DIA, RedStone : pas de change fiat confirmé sur Stellar.
- ✅ `r_USD` = taux des bons du Trésor US tokenisés (~4 %, coût d'opportunité du capital mis en couverture) : cohérent avec `USD 400`, rien à changer dans le code.
- ⬜ (option) `forward_rate` utilise `twap` de Reflector au lieu de `lastprice` pour le fixing de `settle` (parade anti-manipulation citée dans le README). Hors MVP si pas le temps.
- ⬜ `QuoteCard` : afficher le **coût de la protection** (spread + rendement perdu sur la marge ≈ 0,6 % pour 90 j) à côté du prix figé.
- ⬜ `sim/` : y déposer les simulations faites pendant la discussion (copies dans le scratchpad de Claude : `tranches2.py`, `lp_annual.py`), en plus de `pnl.py` (§4.2).
- ⬜ Slides (§6), en plus des deux prévues :
  - « Ce que paie la PME » : ~0,6 % pour 90 j vs forward bancaire 0,5-2 % (avec ligne de crédit) vs option ~2,6 % ;
  - « Comment le pool se couvre » : netting → détention de la devise en CETES (le rendement local paie l'écart forward/spot) → limites ; couverture par portefeuille avec seuil ;
  - « Senior / junior » : senior ~5 % protégé par construction, junior ~16 % avec netting (7,7 % sans) ;
  - « 24 devises dès le mainnet » (liste Reflector).
- ⬜ Objections, à ajouter au §6 :
  - « Comment les LPs se couvrent ? » → netting, puis détention de la devise en obligations d'État tokenisées ; le reste (sauts, écarts oracle/DEX) est petit et payé par le spread ; jamais « couverture totale » ;
  - « Le rendement LP bat-il un vault ? » → senior ≈ bons du Trésor + 1 pt ; le junior n'est bien payé qu'avec du netting, d'où le skew de prix.

## Ordre d'attaque

| Quand | Quoi |
|---|---|
| Maintenant | §0 env · §1.1 `spot` · §1.2 `forward_rate` · §2 tests parité + spread → **push, prévenir Paiement** |
| Dim. soir | §1.3–1.5 `quote`, plafonds, admin · reste des tests · §5 commandes pour Hugo |
| Nuit / lun. 8h | §3.1 `QuoteCard` · §3.2 `CrashButton` · §4.1 `crash.sh` · §6 slides |
| Lun. 8h | Merge avec Hugo, aligner le mock sur Reflector, vérifier le devis dans l'écran Paiement |
| Lun. 8h–10h | §4.2 `sim/`, objections, relecture |
