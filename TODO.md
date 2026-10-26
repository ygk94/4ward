# TODO — HackMeridian

**Deadline : lundi 26/10 à 12h00, sans soumission tardive.** Le jury note de façon asynchrone : **vidéo démo + deck + repo GitHub + README**. Finalistes : pitch live mercredi 28/10.

Légende : ✅ fait · ⬜ à faire · 🔴 bloquant pour les autres ou pour la démo

---

## Qui fait quoi

| | **Vault + Front** — Hugo | **Paiement** — Clément | **Pricing** — Lancelot |
|---|---|---|---|
| Rôle | L'argent du pool, **tout le front**, et le transversal (déploiement, merge, deck, vidéo, soumission) | Le parcours de Lucía côté contrat : ouvrir la couverture, la régler, le paiement du fournisseur | Le prix : oracle, taux forward, spread, plafonds, crash du peso, **rendement du capital libre (Etherfuse / bons du Trésor US)** |
| Contrat | `contracts/pool/src/vault.rs` | `contracts/pool/src/hedge.rs` | `contracts/pool/src/pricing.rs` |
| Front | **tout `front/`** | — (fournit à Hugo le déroulé des appels) | — (fournit à Hugo les appels et formules) |
| Scripts | `scripts/setup-testnet.sh` | — | `scripts/crash.sh` |
| Branche | `feat/vault` (contrat + front) | `feat/payment` | `feat/pricing` |

**On ne modifie pas les fichiers des autres** : on leur demande.

**Dépendances entre vous :**
- Paiement appelle Vault (`reserve`, `release`, `book_gain`, `book_loss`) et Pricing (`spot`, `forward_rate`, `check_cap`, `to_usdc`). Ces fonctions sont **déjà déclarées** : Paiement code contre elles dès maintenant.
- Le front s'appuie sur les points d'entrée publics de `lib.rs`, qui sont figés : Hugo peut construire les écrans tout de suite, avec des données factices en attendant le déploiement.
- `contracts/pool/src/lib.rs` (types partagés et points d'entrée) ne bouge plus, sauf accord à trois. **On ne change pas une signature sans prévenir.**

**Codes d'erreur (`#[contracterror]`) :** Vault 100-199 · Paiement 200-299 · Pricing 300-399. Jamais réutiliser un numéro : le front traduit chaque code en message.

**Conventions communes :**
- prix en USD par unité de devise, **14 décimales** (format Reflector) ;
- montants USDC et notionnels en devise en **7 décimales** (1 000 000 MXN = `10_000_000_000_000`) ;
- plafonds, spread et taux en **bps** (1000 = 10 %) ;
- tout en `i128`, pas de flottants, arrondis en faveur du pool.

**Git :** chacun travaille sur sa branche, `git pull origin main` régulièrement, PR vers `main`. La CI (cargo test + build du front) tourne sur chaque push.

---

## Démarrage (chaque membre)

1. `git clone https://github.com/ygk94/4ward && cd 4ward && git checkout feat/<ta-partie>`
2. `rustup target add wasm32v1-none` · `brew install stellar-cli` (≥ 28) · `stellar contract build && cargo test`
3. Testnet : **chacun monte son propre environnement** avec `./scripts/setup-testnet.sh` (génère tes propres comptes `fx-*` dans ta config `stellar` locale et déploie ton propre pool + mock-oracle). Ensuite `git checkout .env.testnet` : **ne jamais committer ton `.env.testnet`**, celui du repo est le déploiement d'Hugo, celui du front et de la démo. Extension **Freighter** sur **Testnet** si tu veux tester des signatures (`stellar keys secret fx-lucia` → import)
4. Lire `README.md` (le produit) et sa section du tableau ci-dessus

## Déjà en place

- ✅ Repo public `ygk94/4ward`, collaborateurs LancelotWA et cda1712, branches `feat/vault` / `feat/payment` / `feat/pricing`, CI GitHub Actions
- ✅ Contrat `pool` découpé en 3 modules, 10 tests de départ (`#[ignore]`, un bloc par personne)
- ✅ `mock-oracle` : même interface de lecture que Reflector FX, prix pilotable par l'admin
- ✅ Front Vite + React + TS (composants vides par écran), `@stellar/stellar-sdk` 17.4, `@creit.tech/stellar-wallets-kit` 2.7.1
- ✅ Testnet : 4 comptes (`fx-admin`, `fx-lp`, `fx-lucia`, `fx-supplier`, **clés gardées par Hugo, non partagées pour l'instant**), USDC et MXN de test, offre DEX MXN/USDC, **path payment USDC→MXN testé**, mock-oracle et pool déployés. IDs dans `.env.testnet`
- ✅ Reflector FX testnet couvre MXN, BRL, ARS, EUR (`CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W`)
- ✅ README de spec, council du 25/10 dans `docs/council/`

## État au 25/10 soir

- ✅ **Contrat complet et mergé dans `main`** : vault (phases 1 à 6), paiement (`open` / `settle` branchés sur le vault, codes 200-205), pricing (spot, taux forward, plafonds, liste blanche des devises). 143 tests verts (0 ignoré), CI verte. Mergés aussi : fixing à l'échéance, mode démo `time_factor`, `crash.sh`, tranches prudente / dynamique (`deposit_senior`, `withdraw_senior`, `senior_balance`, `tranche_values`, codes 111-112).
- ✅ **Testnet redéployé sur `main`** (IDs dans `.env.testnet`, mode démo 90 j → 5 min) ; scénario exact de la vidéo rejoué : prix figé 55 012,81 USDC (taux Etherfuse figés dans `scripts/rates.snapshot`), crash +12 % (`crash.sh`), règlement au fixing sans rafraîchir le prix, fournisseur payé 1 000 000 MXN, **coût total de Lucía 56 098,72 USDC au centime**. Ne pas relancer `sync-rates.sh --snapshot` avant la vidéo. Tranche prudente testée : dépôt de 100 000 USDC, refus #111 au-delà du ratio.
- ⬜ 🔴 **Front** : en cours (Hugo).
- ⬜ 🔴 **Vidéo, deck, nom, soumission.**
- ⬜ 🔴 **État du testnet avant la vidéo (Hugo, `fx-admin`)** : le mock donne MXN ≈ 0,0692 (crashs du peuplement) ; remettre `set_price --asset '{"Other":"MXN"}' --price 5500000000000` moins de 900 s avant `open_hedge` (pas `crash.sh --reset`, qui prend le cours Reflector), puis vérifier qu'une quote 1e13 / 7776000 / cap 1000 renvoie `usdc_cost` 550128136866.
- ✅ (déploiement précédent, n'existent plus sur le pool actuel) **Couvertures seed 14 (BRL) et 15 (MXN) bloquées en 302** (aucun prix posé avant leur échéance, fixing absent) : marges immobilisées (75 730,32 et 64 410,13 USDC), capacity suffisante pour la démo. Ne pas les montrer dans la vidéo ; ignorer leur bouton Settle dans le front s'il apparaît.

**Points ouverts à trancher :** voir `docs/points-a-trancher.md` (les trois sont réglés).

---

## Vault + Front — Hugo

### Contrat (`vault.rs`)

**Principe :** le vault ne contient **que des dollars** (USDC). Chaque couverture est une jambe devise contre USD ; devises ouvertes en démo : MXN, BRL, ARS, EUR (Reflector testnet couvre aussi GBP, CHF, CAD, THB). Couvertures croisées (MXN → EUR) : plus tard. Le rendement du capital libre (Etherfuse, indexé sur les bons du Trésor US) est **géré par Pricing (Lancelot)** ; le vault lui expose le capital libre.

Checklist cochable : https://claude.ai/artifact/9hWHDQW8siGsCBVb3TZHmY

**Invariant à tenir partout :** `USDC du contrat (+ valeur placée, si Pricing ajoute le rendement) = capital des LPs + Σ marges des couvertures ouvertes`

#### Phase 0 — Préparation
1. ✅ Ne pas toucher `lib.rs` pour le stockage : définir ses propres clés (`enum VaultKey`) et ses erreurs (`#[contracterror] enum VaultError`, codes 100+) dans `vault.rs`
2. ✅ Aide de test qui vérifie l'invariant après chaque opération

#### Phase 1 — Cœur comptable 🔴 (Paiement en dépend)
4. ✅ `reserve(m)` : erreur `InsufficientCapacity` si `m > capacity()`, sinon `Reserved += m`
5. ✅ `release(m)` : `Reserved -= m`
6. ✅ `book_gain(x)` / `book_loss(x)` : capital des LPs ± x (aucun transfert, c'est Paiement qui transfère)
7. ✅ Tests : invariant de solde sur les deux cas types (peso +12 % plafonné, peso −8 %)

#### Phase 2 — Parts LP
8. ✅ Stockage `Shares(Address)` et `TotalShares` ; parts à 7 décimales
9. ✅ `deposit` : `require_auth`, transfert USDC, parts émises = `montant × TotalShares / capital` (1:1 au premier dépôt), arrondi vers le bas
10. ✅ `withdraw` : en parts ou en USDC, avec un **prix de part conservateur** = `(capital − Reserved) / TotalShares` (personne ne sort en laissant ses pertes aux autres), limité à la capacité
11. ✅ Équité des retraits : limite par LP au prorata du capital libre, ou file d'attente (à choisir)
12. ✅ Nouveaux points d'entrée à ajouter dans `lib.rs` (ajout seulement, sans casser l'existant, prévenir l'équipe) : `share_price`, `lp_shares`, `total_assets`
13. ✅ Tests : un gain du pool fait monter le prix de la part ; dépôt après un gain = moins de parts ; retrait limité

#### Phase 3 — Exposition par devise (une devise par couverture pour l'instant)
14. ✅ Suivre l'exposition **nette** par devise contre l'USD : `Exposure(Symbol)` = volume acheteur − volume vendeur, en USD
15. ✅ API pour Paiement : `add_exposure(devise, montant_usd, sens)` / `remove_exposure(...)`, une fois par jambe
16. ⬜ (roadmap) Netting dans la règle de capacité (`Reserved = Σ |exposition nette par devise| × plafond` au lieu de la somme brute des marges) : `basket_max_loss` prêt et testé, non branché ; le MVP garde la règle brute
17. ✅ Limite par devise : Σ marges de la devise ≤ x % du capital (ex. MXN 30 %, ARS 10 %), paramétrable par l'admin
18. ✅ Tests : un acheteur et un vendeur de MXN se compensent ; limite par devise refusée

#### Phase 4 — Rendement du capital libre → confié à Pricing (Lancelot)
Le vault expose `capacity()` et `total_assets()` ; si Lancelot ajoute le placement, se mettre d'accord sur l'interface `invest` / `divest` / `value` dans `vault.rs`.

#### Phase 5 — Robustesse Soroban
27. ✅ `extend_ttl` sur l'instance, les parts et les expositions
28. ✅ Événements : `deposit`, `withdraw`
29. ✅ Multiplications en `checked_mul` (montants × prix en i128), arrondis toujours en faveur du pool
30. ✅ Fonctions admin : `set_currency_limit` (avec `require_auth`)

#### Phase 6 — Intégration
31. ✅ `setup-testnet.sh` : faire le dépôt du LP via `deposit_lp` (et plus seulement un envoi d'USDC)
32. ✅ Mettre à jour le README : vault 100 % USDC, parts LP, règle de capacité

### Front (tout `front/`)
1. ⬜ `shared/wallet.ts` : connexion Freighter via Stellar Wallets Kit (`connect`, `getAddress`, `signTransaction`)
2. ⬜ `shared/contract.ts` : client du pool. Dès que le contrat est déployé : `stellar contract bindings typescript --network testnet --id $POOL_ID --output-dir front/src/shared/pool-client`
3. ⬜ `App.tsx` : mise en page, navigation, bouton de connexion, style général (sobre, zéro jargon : « protection », « prix figé »)
4. ⬜ 🔴 `payment/HedgeScreen.tsx` : **l'écran de Lucía, le cœur de la vidéo**
   - saisie de la facture : montant, devise, échéance (en secondes pour la démo), adresse du fournisseur
   - devis (`QuoteCard`), choix du plafond, bouton « Bloquer mon taux » → `open_hedge`
   - à l'échéance : `settle()` puis **path payment strict receive** USDC → MXN vers le fournisseur, pour exactement le notionnel (2 signatures Freighter)
   - coût total réel de Lucía = prix figé, liens stellar.expert pour les deux transactions
   - comparaison « sans protection / avec protection » après le crash
   - compte à rebours jusqu'à `maturity` ; bouton « Régler » activé seulement après (sinon erreur #202 NotMatured)
   - traduire les codes d'erreur : vault 100-112, paiement 200-206, pricing 300-312, token USDC #10 (solde insuffisant) (tables dans `docs/payment-cli.md` et `docs/todo-pricing.md`)
5. ⬜ `pricing/QuoteCard.tsx` : spot réel Reflector, prix figé, marge à déposer (appel `quote`)
6. ⬜ `pricing/CrashButton.tsx` : `set_price` sur le mock (compte admin), le peso monte de 12 %
7. ⬜ `vault/PoolPanel.tsx` : capital total, réservé, libre, prix de la part, position du LP, **exposition nette par devise** (graphique), indicateur « solvable », déposer / retirer (compte `fx-lp`)

Commencer avec des données factices, brancher le contrat après le merge.

### Infra et intégration
1. ✅ `scripts/setup-testnet.sh` : dépôt LP, paramètres Pricing (taux, plafonds, EUR), une seule offre DEX (bug des offres empilées corrigé le 25/10). Il doit rester utilisable tel quel par Paiement et Pricing sur leur machine (leurs propres comptes)
2. ⬜ **Garder les clés de démo** (`fx-admin`, `fx-lp`, `fx-lucia`, `fx-supplier`) : pas de partage pour l'instant. La démo et la vidéo se font depuis la machine d'Hugo ; tout ce qui demande `fx-admin` sur le déploiement officiel (crash, prix du mock, offre DEX, `set_rate`) passe par Hugo. Partage éventuel plus tard (pitch live du 28/10), en privé, jamais dans le repo
3. ✅ Merge des 3 branches dans `main`, redéploiement et parcours complet vérifié sur testnet (25/10 soir, en avance)
3bis. ⬜ Régénérer les bindings TS pour le front après chaque redéploiement (`stellar contract bindings typescript`), et refaire un merge final lundi matin si Paiement ou Pricing poussent encore
4. ⬜ Déployer le front sur Vercel (prêt : `front/vercel.json`, variables dans `front/.env.example`, marche à suivre dans `front/README.md`) et mettre le lien dans le README

### Pitch et soumission
1. ✅ Nom du projet : **4ward** (26/10), remplacé dans le README, le front, le script vidéo et le mémo ; reste le deck
2. ⬜ Deck (8-10 slides) : problème (Lucía) → solution → démo → pourquoi Stellar → slide « Built on Stellar » en deux colonnes (Unlocked : path payment strict receive USDC → MXN, USDC SAC, événements Soroban ; Composed : Reflector FX SEP-40, Stellar Wallets Kit, taux Etherfuse CETES / USTRY dans le prix à terme) → protection plafonnée et règle de capacité → pricing (slides de Pricing) → concurrence (Hedgehog) → roadmap → équipe
3. ⬜ Slide roadmap : Blend sur le capital non réservé, parts SEP-41, netting, affacturage, SDK anchors
4. ⬜ 🔴 Enregistrer la vidéo (3 min) sur le script de Paiement, **avant lundi 10h**
5. ⬜ README final : nom, captures/GIF, lien vidéo, IDs à jour
6. ⬜ 🔴 Soumission HackMeridian avant 12h : tracks **Stellar Unlocked + Stellar Composed**

## Paiement — Clément

### Contrat (`hedge.rs`) : marche à suivre détaillée en tête du fichier
1. ✅ `settle` en premier, piloté par les tests : hausse (le pool paie), baisse (Lucía paie sur sa marge), au-delà du plafond (borné), garde-fous (pas avant l'échéance, pas deux fois)
2. ✅ `open` : `require_auth(owner)`, `pricing::check_cap`, `locked_rate = pricing::forward_rate`, marge = `to_usdc(notional, locked_rate) × cap_bps / BPS`, transfert de la marge, `vault::reserve(marge)`, stockage + `extend_ttl`
3. ✅ Événements `open` / `settle` (pour le front et l'explorateur)
5. ✅ Branché sur le vault (`open_position` / `close_position`), codes `HedgeError` 200-205
6. ✅ Clôture anticipée `close_early` (26/10) : propriétaire seul, avant l'échéance, au forward du jour pour la durée restante côté inverse (frais de sortie = spread) ; bouton « Close early » avec aperçu du montant dans Mes positions ; 8 tests, testé sur le testnet, pool redéployé
6. ✅ Retirer les `#[ignore]` des 13 tests d'intégration : ajouter `set_rate` / `set_min_cap` dans leur préparation (liste blanche du Pricing)
7. ✅ Régler au **cours de fixing à l'échéance** (`pricing::fixing(currency, maturity)` de Lancelot) au lieu du spot au moment de l'appel : sinon option gratuite pour qui choisit quand régler
4. ✅ (obsolète) Tant que Vault et Pricing ne sont pas prêts : remplacer leurs `panic!("TODO")` par des valeurs en dur en local, **sans les committer**

### Pour le front d'Hugo
1. ✅ (`docs/payment-cli.md`) Valider le parcours en ligne de commande avant le merge : `open_hedge` → `settle` → path payment, sur **ton propre déploiement** (cf. Démarrage), avec les commandes `stellar contract invoke` / `stellar tx new path-payment-strict-receive` exactes, à donner à Hugo. Écrire les commandes avec les alias (`--id fx-pool`, `--source-account fx-lucia`) et non des adresses en dur : Hugo les rejoue telles quelles sur le déploiement officiel
2. ✅ (`docs/payment-cli.md`) Préciser les montants à afficher (marge rendue, P&L, coût total) et les cas d'erreur à gérer
3. ✅ Chiffres de `payment-cli.md` et `video-script.md` mis à jour avec le vrai Pricing (historique, 60 s, taux 400/700 : 55 275 / 56 072,50 ; répétition testnet au centime)
4. ✅ Facteur d'accélération pour la démo (point 2, option b) : prix calculé sur 90 j, échéance réelle courte ; puis repasser la vidéo aux chiffres de 90 j

### Testnet
- ✅ Vérifier la profondeur de l'offre DEX MXN/USDC pour le montant de la démo ; si elle doit être réalignée sur le prix du mock, donner la commande à Hugo (l'offre est faite par `fx-admin`)

### Pitch
- ✅ Script de la vidéo en 5 temps : `docs/video-script.md` (sur `feat/payment`, à merger dans `main`)
- ✅ Expliquer honnêtement les 2 signatures (`settle` puis path payment) : une transaction Soroban ne peut pas contenir d'opération classique ; version atomique en roadmap

## Pricing — Lancelot ⭐ prioritaire

Un prix faux = arbitrage contre le pool : c'est ce qui rend le produit crédible face à un jury finance.

### Contrat (`pricing.rs`)
1. ✅ 🔴 `spot` : client SEP-40 sur l'adresse `Oracle`, `lastprice(Asset::Other(currency))`. Refuser si pas de prix ou prix trop ancien (Reflector : mise à jour toutes les 300 s). Le même code doit marcher sur Reflector **et** sur le mock
2. ✅ 🔴 `forward_rate` : `F = S × (1 + r_USD·t) / (1 + r_dev·t)` en i128 (t = durée / 31 536 000 s), puis le spread **contre l'utilisateur** (BuyCurrency → F × (1 + spread), SellCurrency → F × (1 − spread))
3. ✅ `quote` : `spot`, `forward_rate`, `usdc_cost = to_usdc(notional, F)`, `margin = usdc_cost × cap_bps / BPS`
4. ✅ `check_cap` et `set_min_cap` : plafond minimum par devise (ex. MXN 5 %, BRL 7 %, ARS 15 %)
5. ✅ `set_rate`, `set_spread` (admin, `require_auth`) avec des valeurs réalistes et sourcées : taux Etherfuse figés dans `scripts/rates.snapshot` (USD 3,6 %, MXN 5,55 %, BRL 11,48 %, ARS 22 %, EUR 2,1 %) ; repli en dur historique : USD 4 %, MXN 7 %, BRL 10,5 %, ARS
6. ✅ Tests du bloc Pricing : parité chiffrée (r_MXN 7 %, r_USD 4 %, 90 j → F ≈ 0,05460 ; historique, remplacé par Etherfuse 3,6 % / 5,55 %, F ≈ 0,05474 hors spread), sens du spread, plafond minimum
7. ✅ Donner à Hugo les appels `set_rate` / `set_min_cap` à ajouter au script de setup
8. ✅ Remplacer les `panic!("...")` (`currency not enabled`, `stale price`, `rate not set`…) par `#[contracterror] enum PricingError` (codes 300-399) : les textes sont effacés dans le WASM, le front ne peut pas les distinguer
9. ✅ `fixing(currency, maturity)` pour `settle` : lecture `price(asset, timestamp)` de Reflector ; historique des prix dans le mock-oracle (`price(asset, ts)`)

### Oracle et scripts
1. ✅ `scripts/crash.sh` (`--refresh`, `--reset`, `--dry-run`) : `set_price` du MXN +12 % sur le mock en ligne de commande (filet de sécurité pendant la vidéo). Utiliser les alias (`fx-admin`, `fx-mock-oracle`) : tu le testes sur ton déploiement, Hugo le lance sur le sien
2. ⬜ ⚠️ `spot` refuse un prix de plus de 900 s (`MAX_PRICE_AGE_SECS`) : pendant la démo, le prix du mock doit être frais avant `open_hedge` (le taux forward lit le spot) ; au `settle`, le fixing exige un prix posé moins de 900 s avant l'échéance (le crash le fait). Juste avant la vidéo : donner à Hugo la commande pour aligner le prix du mock sur le vrai spot Reflector (c'est lui qui la lance, avec `fx-admin`) ; prévenir Paiement pour l'offre DEX
3. ⬜ Donner à Hugo la lecture du spot Reflector côté front (contrat, `Asset::Other("MXN")`, conversion des 14 décimales) pour `QuoteCard`

### Rendement du capital libre (Etherfuse, indexé sur les bons du Trésor US)
Touche `vault.rs` : interface à convenir avec Hugo, c'est lui qui l'intègre dans le vault.

0. ⬜ Décision : USDC pur (simple, les LPs ne gagnent que le spread) ou placement USTRY (~4 %/an en plus, KYC et délai de rachat en vrai). Regarder le sandbox d'API Etherfuse (`api.sand.etherfuse.com`)
19. ⬜ Interface d'adaptateur de rendement : `invest(montant)`, `divest(montant)`, `value()` (pour pouvoir changer de source plus tard : Etherfuse, Blend…)
20. ⬜ Contrat `contracts/mock-stablebond` pour le testnet : token USTRY dont la valeur de rachat monte avec le temps (~4 %/an), `buy(usdc)` / `redeem(ustry)`
21. ⬜ `total_assets()` = USDC liquide (hors marges) + quantité d'USTRY × prix ; le capital des LPs devient calculé, et le rendement fait monter le prix de la part
22. ⬜ Coussin de liquidité : garder en USDC au moins `max(Reserved, x % du capital)` pour payer les règlements et les retraits ; n'investir que le surplus
23. ⬜ `rebalance()` permissionless : investit ou désinvestit pour revenir au coussin cible
24. ⬜ Désinvestissement automatique quand un retrait ou un règlement dépasse le liquide
25. ⬜ Tests : le rendement fait monter le prix de la part ; un retrait plus gros que le liquide déclenche `divest` ; l'invariant tient avec une partie investie
26. ⬜ Note pour le pitch et le mainnet : Etherfuse exige un KYC (porté par l'entité qui opère le pool), risque de taux faible sur des bons courts, délai de rachat éventuel

### Pitch
1. ⬜ Slide « le produit est un tunnel de change » : collar symétrique centré sur F, valeur ≈ 0 à l'ouverture, donc le spread rémunère le pool
2. ⬜ `sim/` : notebook P&L de Lucía selon le mouvement du peso, avec et sans protection (graphique pour le deck) ; calibrer spread et plafonds minimum avec la volatilité de chaque devise
3. ⬜ Réponses aux objections : plafond vs mouvements de 15 % · marge immobilisée vs banque · anti-sélection (limites par devise, skew en roadmap) · taux synchronisés depuis Etherfuse (push oracle), lecture onchain en roadmap · réglementation des dérivés (MVP testnet, KYC via anchors en roadmap)

---

## À dire dans la présentation

- **Devises :** devises ouvertes en démo : MXN, BRL, ARS, EUR (Reflector testnet couvre aussi GBP, CHF, CAD, THB). Sur mainnet, toute devise couverte par l'oracle s'ajoute sans changer le contrat.
- **Etherfuse :** pas d'actifs sur le testnet Stellar, donc on émet nos propres actifs de test ; en production, le capital libre passe par Etherfuse (stablebonds indexés sur les bons du Trésor US). Les taux Etherfuse sont **déjà** dans le pricing : rendements USTRY, CETES, TESOURO et EUROB relevés sur l'API Etherfuse (`scripts/sync-rates.sh`), figés dans `scripts/rates.snapshot` et posés par le setup (argument Stellar Composed).
- **Couvertures croisées** (ex. MXN → EUR) : en roadmap. Le modèle « une jambe par devise contre USD » les rend naturelles : chaque jambe compte dans l'exposition de sa devise.

## Démo (vidéo de 3 min)

1. Lucía et sa facture de 1 000 000 MXN à 90 jours, spot 0,055 lu sur l'oracle de démo (mock SEP-40, même interface que Reflector)
2. Plafond 10 %, prix figé, dépôt de marge, signature Freighter
3. « Crash du peso » +12 % : sans protection, la facture coûte 6 587 $ de plus (61 600) ; avec, Lucía ne supporte que ce qui dépasse son plafond (+1 086 $, 56 098,72 au total)
4. Échéance (en secondes) : `settle()` crédite l'USDC, le path payment livre 1 000 000 MXN au fournisseur, visible sur l'explorateur
5. Pool toujours solvable ; slide « suite » : netting, Blend, affacturage, SDK anchors

## Hors MVP (slide roadmap, ne pas coder)

Blend · parts LP SEP-41 · netting par devise/échéance · skew de prix · facture transférable / affacturage · SDK anchors · taux d'intérêt onchain · TWAP au fixing · paiement atomique couverture + path payment

## Planning indicatif

| Créneau | Vault + Front (Hugo) | Paiement | Pricing |
|---|---|---|---|
| Dim. soir | `reserve`/`release`, deposit/withdraw + tests, `wallet.ts`, nom | `settle` + tests, script vidéo | `spot` + `forward_rate` + tests |
| Nuit / lun. 8h | Écran Lucía + devis + crash (factice), panneau pool | `open`, parcours validé en CLI | `quote`, plafonds, `crash.sh`, slides pricing |
| Lun. 8h | **Merge, redéploiement, bindings TS, branchement du front** | Parcours complet rejoué chez Hugo (commandes fournies) | Commandes d'alignement des prix fournies à Hugo |
| Lun. 8h-10h | Vidéo | Aide au polish / deck | Notebook `sim/`, objections |
| Lun. 10h-12h | README final + **soumission** | Relecture | Relecture |

---

## Améliorations issues du council (après le MVP)

Points soulevés par le council que le MVP ne traite pas. À reprendre une fois la soumission faite, ou plus tôt s'il reste du temps. Chaque point peut aussi servir de réponse au jury (« on l'a vu, voici la parade »).

**Produit**
- ⬜ **Paiement atomique couverture + fournisseur.** Aujourd'hui, `settle` et le path payment font 2 transactions. Piste : faire le swap USDC → devise depuis le contrat via un DEX Soroban (Soroswap, par exemple) pour régler et payer dans une seule transaction
- ⬜ **Rendre la marge productive.** La marge immobilisée est l'objection n° 1 face à la banque : la placer sur Blend pendant la durée de la couverture, avec un rendement reversé à la PME
- ⬜ **Persona exportateur** (`SellCurrency`) dans la démo, pour montrer que des flux opposés se compensent (le netting raconté par une histoire)
- ⬜ **Facture couverte transférable** (token SEP-41) : vendre une créance déjà couverte à un financeur (affacturage)

**Risque et pricing**
- ⬜ **Anti-sélection** : limites d'exposition par devise, skew de prix (le côté majoritaire paie plus cher), spread dynamique autour des événements connus (banques centrales)
- ⬜ **Netting réel** par devise et par échéance dans la règle de capacité (aujourd'hui, règle brute plus prudente)
- ⬜ **Fixing robuste** : TWAP sur une fenêtre au lieu du dernier prix, bornes de cohérence, plusieurs oracles
- ⬜ **Taux d'intérêt onchain** au lieu des taux codés en dur
- ⬜ **Plafonds asymétriques** : les pricer comme des options (volatilité), et plus seulement avec un spread
- ⬜ **Écart oracle / cours réel** : fixing sur les cotations SEP-38 des anchors

**Pool**
- ⬜ **Parts LP** (SEP-41) avec répartition du P&L
- ⬜ **Blend** sur le capital non réservé du pool
- ⬜ **Amorçage de la liquidité** : incitations pour les premiers LPs, rendement affiché

**Pitch et marché**
- ⬜ **Test des 20 secondes** : expliquer le projet à quelqu'un hors finance ; chaque mot qui suscite une question sort du pitch
- ⬜ **Chiffrer face à la banque** : coût de la couverture (spread + coût de la marge) comparé à un forward bancaire et à « aucune couverture »
- ⬜ **Réglementation** : un NDF vendu à des PME est un dérivé réglementé (EMIR, MiFID) → KYC via les anchors (SEP-12), passage par une entité régulée
- ⬜ **Distribution** : SDK « Bloquer le taux » pour wallets et anchors
- ⬜ **Couverture Reflector mainnet** : vérifier la liste des devises (seul le testnet a été vérifié)
