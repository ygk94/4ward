# TODO Pricing — détaillé

> Branche `feat/pricing`. Fichiers à nous : `contracts/pool/src/pricing.rs`, bloc Pricing de `contracts/pool/src/test.rs`, `front/src/pricing/`, `scripts/crash.sh`, `sim/`.
> On ne touche pas à `lib.rs`, `hedge.rs`, `vault.rs`, `shared/` : on demande.
> Deadline : **lundi 26/10 12h00**. Priorité absolue : `spot` + `forward_rate` + tests (bloque Paiement).

Légende : ⬜ à faire · 🔴 bloque quelqu'un d'autre · ✅ fait

---

## 0. Environnement (avant tout)

- ⬜ `curl https://sh.rustup.rs -sSf | sh` puis `rustup target add wasm32v1-none` (Rust ≥ 1.84)
- ⬜ `brew install stellar-cli` (≥ 28) · `brew install pnpm` (ou `corepack enable`)
- ⬜ `cargo test` à la racine : tout doit compiler (10 tests `#[ignore]`)
- ⬜ `stellar contract build` : wasm de `pool` et `mock_oracle` dans `target/wasm32v1-none/release/`
- ⬜ Récupérer `front/.env.local` auprès d'Hugo (ou `./scripts/setup-testnet.sh`), `cd front && pnpm install && pnpm dev`
- ⬜ Freighter en Testnet, importer `fx-admin` (nécessaire pour le bouton crash) et `fx-lucia`

---

## 1. Contrat `pricing.rs`

### 1.1 🔴 `spot(env, currency) -> i128`
Doit marcher **à l'identique** sur le mock-oracle et sur Reflector FX.
- ⬜ Déclarer un client SEP-40 minimal via `soroban_sdk::contractclient` :
  ```rust
  #[contracttype] pub enum Asset { Stellar(Address), Other(Symbol) }
  #[contracttype] pub struct PriceData { pub price: i128, pub timestamp: u64 }
  #[contractclient(name = "OracleClient")]
  pub trait Sep40 { fn lastprice(env: Env, asset: Asset) -> Option<PriceData>; fn decimals(env: Env) -> u32; }
  ```
  Les types doivent avoir exactement les mêmes noms/variantes que ceux du mock (`Asset::Other(Symbol)`, `PriceData{price,timestamp}`) pour que l'encodage XDR coïncide.
- ⬜ Lire `DataKey::Oracle` dans l'instance, appeler `lastprice(Asset::Other(currency))`
- ⬜ `None` → `panic!("no price")` ; `price <= 0` → panic
- ⬜ Fraîcheur : `env.ledger().timestamp() - timestamp > MAX_AGE` → `panic!("stale price")`.
  Reflector met à jour toutes les 300 s → `MAX_AGE = 900` (3 résolutions) laisse de la marge. Attention : le mock horodate au `set_price`, donc en test il faut avancer le ledger avec `env.ledger().set_timestamp(...)` **avant** `set_price`, ou re-setter le prix.
- ⬜ (option) vérifier `decimals() == 14` une fois, ou documenter l'hypothèse. Reflector FX = 14, mock = 14.

### 1.2 🔴 `forward_rate(env, currency, side, duration_secs) -> i128`
```
F = S × (1 + r_USD·t) / (1 + r_dev·t)      t = duration_secs / 31_536_000
```
- ⬜ Tout en `i128`, en bps. Avec `SECS_PER_YEAR = 31_536_000` et `BPS = 10_000` :
  ```
  num = BPS × SECS_PER_YEAR + r_usd_bps × duration
  den = BPS × SECS_PER_YEAR + r_dev_bps × duration
  F   = S × num / den
  ```
  Ordre des opérations : multiplier **avant** de diviser (S ≈ 5,5e12, num ≈ 3,2e11 → 1,8e24, largement sous i128::MAX ≈ 1,7e38 ; vérifier avec un spot BRL/EUR ≈ 1e14 et une durée d'1 an : toujours OK).
- ⬜ Taux lus dans `DataKey::Rate(currency)` et `DataKey::Rate("USD")` ; absent → panic `"rate not set"` (pas de défaut silencieux : un taux manquant = prix faux = arbitrage).
- ⬜ Spread **contre l'utilisateur**, lu dans `DataKey::SpreadBps` :
  - `BuyCurrency` (Lucía paie la devise) → `F × (BPS + spread) / BPS`
  - `SellCurrency` → `F × (BPS − spread) / BPS`
- ⬜ Arrondis en faveur du pool : `BuyCurrency` → arrondir **vers le haut** (ceil : `(a + b − 1) / b`), `SellCurrency` → arrondir vers le bas (division entière). Factoriser un helper `mul_div_ceil` / `mul_div_floor`.
- ⬜ `duration_secs == 0` → F = S (autorisé pour la démo en secondes). Durée > 1 an → panic (hors produit).

### 1.3 `quote(...) -> Quote`
- ⬜ `spot`, `forward_rate`, `usdc_cost = to_usdc(notional, forward_rate)`, `margin = usdc_cost × cap_bps / BPS` (ceil)
- ⬜ Appeler `check_cap` dedans pour que le front ait l'erreur avant `open_hedge`
- ⬜ `notional <= 0` → panic
- ⬜ Vérifier que `to_usdc` est cohérent : 10_000_000_000_000 (1 M MXN, 7 déc.) × 5_500_000_000_000 (0,055, 14 déc.) / 1e14 = 550_000_000_000 = 55 000 USDC ✓. Proposer à Hugo de passer `to_usdc` en ceil (c'est son fichier ? non, il est dans `pricing.rs` : à nous).

### 1.4 `check_cap` / `set_min_cap`
- ⬜ `set_min_cap` : `admin(env).require_auth()`, `cap_bps ≤ BPS`, stocker `DataKey::MinCap(currency)` en instance
- ⬜ `check_cap` : `cap_bps < MinCap(currency)` → panic `"cap below minimum"` ; devise sans minimum → panic (devise non activée = refusée, c'est la liste blanche des devises) ; `cap_bps > 5000` → panic (plafond absurde)
- ⬜ Valeurs : MXN 500 (5 %), BRL 700 (7 %), ARS 1500 (15 %), EUR 300 (3 %) — à calibrer avec `sim/` (§4)

### 1.5 `set_rate` / `set_spread`
- ⬜ `admin(env).require_auth()` dans les deux
- ⬜ `set_rate(currency, bps)` : `bps ≤ 10_000` (100 %/an, ARS peut être haut), stocker `Rate(currency)`
- ⬜ `set_spread(bps)` : `bps ≤ 500`
- ⬜ Valeurs sourcées (noter la source dans un commentaire + dans le deck) :
  | Devise | Taux directeur ~oct. 2026 | bps |
  |---|---|---|
  | USD | Fed funds ≈ 4 % | 400 |
  | MXN | Banxico ≈ 7 % | 700 |
  | BRL | Selic ≈ 10,5 % | 1050 |
  | ARS | BCRA, à vérifier (≈ 30–40 %) | ~3500 |
  | EUR | BCE ≈ 2 % | 200 |
  → vérifier les chiffres réels avant la vidéo, les mettre dans `scripts/setup-testnet.sh` via Hugo (§5).

### 1.6 `extend_ttl`
- ⬜ Tout est en storage `instance` (Rate, MinCap, Spread) : `env.storage().instance().extend_ttl(...)` dans `quote` suffit (ou laisser à Vault qui le fait sur l'instance). À demander à Hugo pour éviter le doublon.

---

## 2. Tests (bloc Pricing de `test.rs`)

Mise en place commune (helper dans le bloc Pricing, ne pas toucher aux autres blocs) :
- ⬜ `env.mock_all_auths()`, enregistrer le mock-oracle **par son wasm** (`contractimport!(file = "../../target/wasm32v1-none/release/mock_oracle.wasm")`) ou par crate `mock-oracle` en dev-dependency (plus simple : `[dev-dependencies] mock-oracle = { path = "../mock-oracle" }` puis `env.register(mock_oracle::MockOracle, ...)` — demander à Hugo pour le `Cargo.toml` de `pool`, c'est un fichier partagé). Préférer le `contractimport!` car la CI fait `cargo test` sans `stellar contract build`… donc **la dev-dependency est la bonne option** pour la CI.
- ⬜ `set_price(MXN, 5_500_000_000_000)`, `set_rate(USD, 400)`, `set_rate(MXN, 700)`, `set_spread(50)`, `set_min_cap(MXN, 500)`

Tests à retirer de `#[ignore]` :
- ⬜ `pricing_forward_parity` : spot 0,055, 90 j (7_776_000 s), spread 0 → F = 5_500_000_000_000 × (10_000×31_536_000 + 400×7_776_000) / (10_000×31_536_000 + 700×7_776_000). Calculer la valeur exacte attendue en i128 (≈ 5_459_7xx_xxx_xxx, soit ≈ 0,05460) et l'assert à l'unité près (pas de « ≈ »).
- ⬜ `pricing_spread_direction` : avec spread 50 bps, `Buy ≥ F_sans_spread`, `Sell ≤ F_sans_spread`, et `Buy − F ≈ F × 0,5 %` ; vérifier aussi que ceil/floor ne se croisent jamais (`Sell < Buy`).
- ⬜ `pricing_min_cap` : `check_cap(MXN, 400)` panique (`#[should_panic]`), `500` passe, devise inconnue panique.
- ⬜ Nouveaux tests (ajouter au bloc) :
  - `pricing_quote_amounts` : 1 M MXN, cap 10 % → `usdc_cost` ≈ 54 870 USDC avec spread, `margin` = 10 % de ça, cohérent avec le README (« vous paierez 54 870 USDC »). Recalculer le chiffre exact et **mettre à jour le README si ça diffère** (demander à Hugo).
  - `pricing_stale_price_panics` : `set_price`, puis `env.ledger().set_timestamp(now + 901)`, `spot` panique.
  - `pricing_no_price_panics` : devise sans prix → panic.
  - `pricing_rate_missing_panics` : `forward_rate` sans `set_rate` → panic.
  - `pricing_set_rate_requires_admin` : sans `mock_all_auths` → échec d'auth.
  - `pricing_duration_zero_is_spot` : F(0 s) = S ± spread.
- ⬜ `cargo test -p pool` vert, CI verte sur le push.

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
- ⬜ Rendu, zéro jargon : « Prix figé : vous paierez **54 870 USDC** quoi qu'il arrive » · « Protection ±10 % » · « Dépôt de garantie : 5 487 USDC (restitué à l'échéance, ± l'écart) » · petit texte « taux à terme = cours du jour ajusté de l'écart de taux MXN 7 % / USD 4 % ».
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

- ⬜ 🔴 À Hugo, pour `scripts/setup-testnet.sh` (dès que §1.5 est fixé) :
  ```sh
  for r in "USD 400" "MXN 700" "BRL 1050" "ARS 3500"; do set -- $r
    stellar contract invoke --id $POOL --source-account $ADMIN --network $NET --quiet -- set_rate --currency $1 --rate_bps $2; done
  for c in "MXN 500" "BRL 700" "ARS 1500"; do set -- $c
    stellar contract invoke --id $POOL --source-account $ADMIN --network $NET --quiet -- set_min_cap --currency $1 --cap_bps $2; done
  ```
- ⬜ À Hugo : dev-dependency `mock-oracle` dans `contracts/pool/Cargo.toml` (ou accord pour que je l'ajoute)
- ⬜ À Paiement : props de `<QuoteCard />` figées + `onQuote` ; signaler tout changement du prix mock (offre DEX)
- ⬜ À Hugo : chiffre exact de `usdc_cost` pour le README (54 870 ?) et les 2 slides pricing

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

## Ordre d'attaque

| Quand | Quoi |
|---|---|
| Maintenant | §0 env · §1.1 `spot` · §1.2 `forward_rate` · §2 tests parité + spread → **push, prévenir Paiement** |
| Dim. soir | §1.3–1.5 `quote`, plafonds, admin · reste des tests · §5 commandes pour Hugo |
| Nuit / lun. 8h | §3.1 `QuoteCard` · §3.2 `CrashButton` · §4.1 `crash.sh` · §6 slides |
| Lun. 8h | Merge avec Hugo, aligner le mock sur Reflector, vérifier le devis dans l'écran Paiement |
| Lun. 8h–10h | §4.2 `sim/`, objections, relecture |
