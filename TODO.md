# TODO — HackMeridian

**Deadline : lundi 26/10 à 12h00, sans soumission tardive.** Le jury note de façon asynchrone : **vidéo démo + deck + repo GitHub + README**. Finalistes : pitch live mercredi 28/10.

Légende : ✅ fait · ⬜ à faire · 🔴 bloquant pour les autres ou pour la démo

---

## Qui fait quoi

| | **Vault** — Hugo | **Paiement** — ? | **Pricing** — ? |
|---|---|---|---|
| Rôle | L'argent du pool **+ tout le transversal** (front commun, déploiement, merge, deck, vidéo, soumission) | Le parcours de Lucía : ouvrir la couverture, la régler, payer le fournisseur | Le prix : oracle, taux forward, spread, plafonds, crash du peso |
| Contrat | `contracts/pool/src/vault.rs` | `contracts/pool/src/hedge.rs` | `contracts/pool/src/pricing.rs` |
| Front | `front/src/shared/`, `App.tsx`, `front/src/vault/` | `front/src/payment/` | `front/src/pricing/` |
| Scripts | `scripts/setup-testnet.sh` | — | `scripts/crash.sh` |
| Branche | `feat/vault` | `feat/payment` | `feat/pricing` |

Chaque fichier commence par un tag `[VAULT]`, `[PAIEMENT]` ou `[PRICING]` qui indique son responsable. **On ne modifie pas les fichiers des autres** : on leur demande.

**Dépendances entre vous :**
- Paiement appelle Vault (`reserve`, `release`, `book_gain`, `book_loss`) et Pricing (`spot`, `forward_rate`, `check_cap`, `to_usdc`). Ces fonctions sont **déjà déclarées** : Paiement code contre elles dès maintenant.
- Paiement utilise `<QuoteCard />` de Pricing dans son écran.
- Les trois écrans utilisent `shared/wallet.ts` et `shared/contract.ts` du Vault → 🔴 **le Vault les livre en premier**.
- `contracts/pool/src/lib.rs` (types partagés et points d'entrée) ne bouge plus, sauf accord à trois. **On ne change pas une signature sans prévenir.**

**Conventions communes :**
- prix en USD par unité de devise, **14 décimales** (format Reflector) ;
- montants USDC et notionnels en devise en **7 décimales** (1 000 000 MXN = `10_000_000_000_000`) ;
- plafonds, spread et taux en **bps** (1000 = 10 %) ;
- tout en `i128`, pas de flottants, arrondis en faveur du pool.

**Git :** chacun travaille sur sa branche, `git pull origin main` régulièrement, PR vers `main`. La CI (cargo test + build du front) tourne sur chaque push.

---

## Démarrage (chaque membre)

1. `git clone https://github.com/ygk94/fistea-hackmeridian && cd fistea-hackmeridian && git checkout feat/<ta-partie>`
2. `rustup target add wasm32v1-none` · `brew install stellar-cli` (≥ 28) · `stellar contract build && cargo test`
3. `cd front && pnpm install && pnpm dev`. Demander à Hugo le fichier `front/.env.local` (IDs testnet), ou lancer soi-même `./scripts/setup-testnet.sh`
4. Extension **Freighter**, réseau **Testnet**, importer les comptes de démo reçus d'Hugo
5. Lire `README.md` (le produit) et sa section du tableau ci-dessus

## Déjà en place

- ✅ Repo public `ygk94/fistea-hackmeridian`, collaborateurs LancelotWA et cda1712, branches `feat/vault` / `feat/payment` / `feat/pricing`, CI GitHub Actions
- ✅ Contrat `pool` découpé en 3 modules, 10 tests de départ (`#[ignore]`, un bloc par personne)
- ✅ `mock-oracle` : même interface de lecture que Reflector FX, prix pilotable par l'admin
- ✅ Front Vite + React + TS découpé par personne (composants vides), `@stellar/stellar-sdk` 17.4, `@creit.tech/stellar-wallets-kit` 2.7.1
- ✅ Testnet : 4 comptes (`fx-admin`, `fx-lp`, `fx-lucia`, `fx-supplier`), USDC et MXN de test, LP à 1 M USDC, Lucía à 200 k USDC, offre DEX MXN/USDC, **path payment USDC→MXN testé**, mock-oracle et pool déployés. IDs dans `.env.testnet`
- ✅ Reflector FX testnet couvre MXN, BRL, ARS, EUR (`CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W`)
- ✅ README de spec, council du 25/10 dans `docs/council/`
- ✅ Checklist des tâches externes d'Hugo (wallet, accès, hackathon, outils) : `docs/setup-externe.html`

---

## Vault — Hugo

### Contrat (`vault.rs`)
1. ⬜ `deposit` : `require_auth(lp)`, transfert USDC lp → contrat, `LpBalance += amount`, `TotalDeposits += amount`
2. ⬜ `withdraw` : `require_auth(lp)`, `amount ≤ solde` et `amount ≤ capacity()`, transfert contrat → lp
3. ⬜ 🔴 `reserve` / `release` : tenir `Reserved` ; `reserve` panique si `amount > capacity()` (règle « le pool ne peut pas faire faillite »)
4. ⬜ `book_gain` / `book_loss` : faire bouger `TotalDeposits` avec le P&L des couvertures
5. ⬜ Décider de la répartition du P&L entre LPs : mapping simple (suffisant pour la démo) ou parts proportionnelles
6. ⬜ `extend_ttl` sur l'instance et les soldes LP
7. ⬜ Tests du bloc Vault dans `test.rs`

### Front commun (transversal)
1. ⬜ 🔴 `shared/wallet.ts` : connexion Freighter via Stellar Wallets Kit (`connect`, `getAddress`, `signTransaction`)
2. ⬜ 🔴 `shared/contract.ts` : client du pool. Dès que le contrat est déployé : `stellar contract bindings typescript --network testnet --id $POOL_ID --output-dir front/src/shared/pool-client`
3. ⬜ `App.tsx` : mise en page, navigation, bouton de connexion, style général (sobre, zéro jargon : « protection », « prix figé »)
4. ⬜ `vault/PoolPanel.tsx` : dépôts totaux, capital réservé, capacité restante, indicateur « solvable », déposer / retirer (compte `fx-lp`)

### Infra et intégration
1. ⬜ Maintenir `scripts/setup-testnet.sh` (y intégrer le `set_rate` / `set_min_cap` fournis par Pricing)
2. ⬜ 🔴 **Lundi 8h : merge des 3 branches**, redéploiement, régénération des bindings, vérification du parcours complet
3. ⬜ (option) Déployer le front (Vercel / Netlify) pour mettre un lien dans le README

### Pitch et soumission
1. ⬜ 🔴 Nom du projet → remplacer `<NOM>` partout (README, front, deck)
2. ⬜ Deck (8-10 slides) : problème (Lucía) → solution → démo → pourquoi Stellar → protection plafonnée et règle de capacité → pricing (slides de Pricing) → concurrence (Hedgehog) → roadmap → équipe
3. ⬜ Slide roadmap : Blend sur le capital non réservé, parts SEP-41, netting, affacturage, SDK anchors
4. ⬜ 🔴 Enregistrer la vidéo (3 min) sur le script de Paiement, **avant lundi 10h**
5. ⬜ README final : nom, captures/GIF, lien vidéo, IDs à jour
6. ⬜ 🔴 Soumission HackMeridian avant 12h : tracks **Stellar Unlocked + Stellar Composed**

## Paiement — ?

### Contrat (`hedge.rs`) : marche à suivre détaillée en tête du fichier
1. ⬜ `settle` en premier, piloté par les tests : hausse (le pool paie), baisse (Lucía paie sur sa marge), au-delà du plafond (borné), garde-fous (pas avant l'échéance, pas deux fois)
2. ⬜ `open` : `require_auth(owner)`, `pricing::check_cap`, `locked_rate = pricing::forward_rate`, marge = `to_usdc(notional, locked_rate) × cap_bps / BPS`, transfert de la marge, `vault::reserve(marge)`, stockage + `extend_ttl`
3. ⬜ Événements `open` / `settle` (pour le front et l'explorateur)
4. ⬜ Tant que Vault et Pricing ne sont pas prêts : remplacer leurs `panic!("TODO")` par des valeurs en dur en local, **sans les committer**

### Front (`payment/HedgeScreen.tsx`) : le cœur de la vidéo
1. ⬜ Saisie de la facture : montant, devise, échéance (en secondes pour la démo), adresse du fournisseur
2. ⬜ Devis via `<QuoteCard />` (Pricing), choix du plafond, bouton « Bloquer mon taux » → `open_hedge`
3. ⬜ À l'échéance : `settle()` puis **path payment strict receive** USDC → MXN vers le fournisseur, pour exactement le notionnel
4. ⬜ Afficher le coût total réel de Lucía = prix figé (aux arrondis et au spread du DEX près)
5. ⬜ Liens stellar.expert (testnet) pour les deux transactions
6. ⬜ Comparaison « sans protection / avec protection » après le crash

### Testnet
- ⬜ Vérifier la profondeur de l'offre DEX MXN/USDC pour le montant de la démo ; la réaligner sur le prix du mock si Pricing le change

### Pitch
- ⬜ 🔴 Script de la vidéo en 5 temps (voir § Démo), à livrer à Hugo dimanche soir
- ⬜ Expliquer honnêtement les 2 signatures (`settle` puis path payment) : une transaction Soroban ne peut pas contenir d'opération classique ; version atomique en roadmap

## Pricing — ? ⭐ prioritaire

Un prix faux = arbitrage contre le pool : c'est ce qui rend le produit crédible face à un jury finance.

### Contrat (`pricing.rs`)
1. ⬜ 🔴 `spot` : client SEP-40 sur l'adresse `Oracle`, `lastprice(Asset::Other(currency))`. Refuser si pas de prix ou prix trop ancien (Reflector : mise à jour toutes les 300 s). Le même code doit marcher sur Reflector **et** sur le mock
2. ⬜ 🔴 `forward_rate` : `F = S × (1 + r_USD·t) / (1 + r_dev·t)` en i128 (t = durée / 31 536 000 s), puis le spread **contre l'utilisateur** (BuyCurrency → F × (1 + spread), SellCurrency → F × (1 − spread))
3. ⬜ `quote` : `spot`, `forward_rate`, `usdc_cost = to_usdc(notional, F)`, `margin = usdc_cost × cap_bps / BPS`
4. ⬜ `check_cap` et `set_min_cap` : plafond minimum par devise (ex. MXN 5 %, BRL 7 %, ARS 15 %)
5. ⬜ `set_rate`, `set_spread` (admin, `require_auth`) avec des valeurs réalistes et sourcées : USD 4 %, MXN 7 %, BRL 10,5 %, ARS
6. ⬜ Tests du bloc Pricing : parité chiffrée (r_MXN 7 %, r_USD 4 %, 90 j → F ≈ 0,05460), sens du spread, plafond minimum
7. ⬜ Donner à Hugo les appels `set_rate` / `set_min_cap` à ajouter au script de setup

### Front (`pricing/`)
1. ⬜ `QuoteCard.tsx` : spot réel Reflector, prix figé, marge à déposer (utilisé par l'écran de Paiement)
2. ⬜ `CrashButton.tsx` : `set_price` sur le mock (compte admin), le peso monte de 12 %
3. ⬜ `scripts/crash.sh` : même chose en ligne de commande (filet de sécurité pendant la vidéo)
4. ⬜ Juste avant la vidéo : aligner le prix du mock sur le vrai spot Reflector (et prévenir Paiement pour l'offre DEX)

### Pitch
1. ⬜ Slide « le produit est un tunnel de change » : collar symétrique centré sur F, valeur ≈ 0 à l'ouverture, donc le spread rémunère le pool
2. ⬜ `sim/` : notebook P&L de Lucía selon le mouvement du peso, avec et sans protection (graphique pour le deck) ; calibrer spread et plafonds minimum avec la volatilité de chaque devise
3. ⬜ Réponses aux objections : plafond vs mouvements de 15 % · marge immobilisée vs banque · anti-sélection (limites par devise, skew en roadmap) · taux d'intérêt en dur (lecture onchain en roadmap) · réglementation des dérivés (MVP testnet, KYC via anchors en roadmap)

---

## Démo (vidéo de 3 min)

1. Lucía et sa facture de 1 000 000 MXN à 90 jours, spot réel lu sur Reflector
2. Plafond 10 %, prix figé, dépôt de marge, signature Freighter
3. « Crash du peso » +12 % : sans protection, la facture coûte ~6 600 $ de plus ; avec, Lucía ne supporte que les 2 % au-delà de son plafond (~1 100 $)
4. Échéance (en secondes) : `settle()` crédite l'USDC, le path payment livre 1 000 000 MXN au fournisseur, visible sur l'explorateur
5. Pool toujours solvable ; slide « suite » : netting, Blend, affacturage, SDK anchors

## Hors MVP (slide roadmap, ne pas coder)

Blend · parts LP SEP-41 · netting par devise/échéance · skew de prix · facture transférable / affacturage · SDK anchors · taux d'intérêt onchain · TWAP au fixing · paiement atomique couverture + path payment

## Planning indicatif

| Créneau | Vault (Hugo) | Paiement | Pricing |
|---|---|---|---|
| Dim. soir | `wallet.ts`, `reserve`/`release`, deposit/withdraw + tests, nom | `settle` + tests, script vidéo | `spot` + `forward_rate` + tests |
| Nuit / lun. 8h | `App.tsx`, panneau pool, deck | `open`, écran Lucía, path payment | `quote`, plafonds, `QuoteCard`, crash, slides pricing |
| Lun. 8h | **Merge, redéploiement, bindings TS** | Tests du parcours complet | Alignement des prix |
| Lun. 8h-10h | Vidéo | Polish de l'écran | Notebook `sim/`, objections |
| Lun. 10h-12h | README final + **soumission** | Relecture | Relecture |
