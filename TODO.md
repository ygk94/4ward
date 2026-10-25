# TODO — HackMeridian

**Deadline : lundi 26/10 à 12h00, sans soumission tardive.** Le jury note de façon asynchrone : **vidéo démo + deck + repo GitHub + README**. Finalistes : pitch live mercredi 28/10.

Légende : ✅ fait · ⬜ à faire · 🔴 bloquant pour la démo

---

## Répartition

| | Personne 1 — **Vault** | Personne 2 — **Paiement** | Personne 3 — **Pricing** |
|---|---|---|---|
| Fichier contrat | `contracts/pool/src/vault.rs` | `contracts/pool/src/hedge.rs` | `contracts/pool/src/pricing.rs` |
| Rôle | L'argent des LPs : dépôts, retraits, capital réservé, solvabilité | Le parcours de Lucía : ouvrir la couverture, la régler, payer le fournisseur | Le prix : spot oracle, taux forward, spread, plafonds |
| Front | Panneau pool / LP | Écran Lucía : blocage du taux, règlement, path payment | Affichage du devis + bouton « crash du peso » |
| Hors code | Coordination vidéo + deck | Script de la démo | Slides pricing et risque, réponses aux objections |
| Branche | `feat/vault` | `feat/payment` | `feat/pricing` |

**Qui dépend de qui :** Paiement appelle Vault (`reserve`, `release`, `book_gain`, `book_loss`) et Pricing (`spot`, `forward_rate`, `check_cap`, `to_usdc`). Ces fonctions sont **déjà déclarées** avec leur signature dans chaque fichier : Paiement peut coder contre elles tout de suite, sans attendre les autres. **On ne change pas une signature sans prévenir.**

Les types partagés (`Hedge`, `Quote`, `Side`, `DataKey`) et les points d'entrée publics sont dans `lib.rs` : ce fichier ne bouge plus, sauf accord à trois.

**Conventions communes :**
- prix en USD par unité de devise, **14 décimales** (format Reflector) ;
- montants USDC et notionnels en devise en **7 décimales** (1 000 000 MXN = `10_000_000_000_000`) ;
- plafonds, spread et taux en **bps** (1000 = 10 %) ;
- tout en `i128`, pas de flottants, arrondis en faveur du pool.

---

## 0. Pour toute l'équipe, maintenant

- ⬜ 🔴 **Nom du projet** (propositions du council : Pinned, Locktide, Fixa) → remplacer `<NOM>` partout
- ⬜ 🔴 Attribuer les 3 rôles
- ⬜ Cloner le repo, `rustup target add wasm32v1-none`, `stellar` CLI ≥ 28 (`brew install stellar-cli`), `stellar contract build && cargo test`
- ⬜ **Freighter** (extension navigateur) → Settings → Network : **Testnet**
- ⬜ Récupérer les clés de démo `fx-admin`, `fx-lp`, `fx-lucia`, `fx-supplier` (export `stellar keys secret <nom>` depuis la machine qui a lancé le setup, à partager par gestionnaire de mots de passe, **jamais dans le repo**)
- ⬜ Ajouter dans Freighter les actifs de test `USDC` et `MXN` (émetteur `GBRF7DL674464UYOUX5IIXELPGK2OYHVWSLX7GDNR5NO2R6FSGGFQPWL`)
- ⬜ Figer les chiffres de la démo : Lucía, 1 000 000 MXN, plafond 10 %, peso +12 %

### Déjà en place

- ✅ Workspace Soroban, contrat `pool` découpé en 3 modules, 10 tests de départ (`#[ignore]`, un bloc par personne)
- ✅ `mock-oracle` : même interface de lecture que Reflector FX, prix pilotable par l'admin
- ✅ Front Vite + React + TS, `@stellar/stellar-sdk` 17.4, `@creit.tech/stellar-wallets-kit` 2.7.1
- ✅ Testnet (`./scripts/setup-testnet.sh`, relançable) : 4 comptes, USDC et MXN de test, LP à 1 M USDC, Lucía à 200 k USDC, offre DEX MXN/USDC, **path payment USDC→MXN testé**, mock-oracle et pool déployés. IDs dans `.env.testnet` et `front/.env.local`
- ✅ Reflector FX testnet couvre MXN, BRL, ARS, EUR (`CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W`)

---

## Personne 1 — Vault

**Contrat (`vault.rs`)**
1. ⬜ `deposit` : `require_auth(lp)`, transfert USDC lp → contrat, `LpBalance += amount`, `TotalDeposits += amount`
2. ⬜ `withdraw` : `require_auth(lp)`, `amount ≤ solde` et `amount ≤ capacity()`, transfert contrat → lp
3. ⬜ `reserve` / `release` : tenir `Reserved`, `reserve` panique si `amount > capacity()` (c'est la règle « le pool ne peut pas faire faillite »)
4. ⬜ `book_gain` / `book_loss` : faire bouger `TotalDeposits` avec le P&L des couvertures
5. ⬜ **Décision à prendre :** comment le P&L se répartit entre LPs. Mapping simple (le P&L reste au niveau du pool, suffisant pour la démo) ou parts proportionnelles (`shares`), plus juste mais plus long
6. ⬜ `extend_ttl` sur l'instance et les soldes LP
7. ⬜ Tests du bloc Vault dans `test.rs`

**Front**
- ⬜ Panneau pool : dépôts totaux, capital réservé, capacité restante, indicateur « solvable », bouton déposer / retirer (compte `fx-lp`)

**Hors code**
- ⬜ Coordination de la vidéo (3 min) et du deck, enregistrement **avant lundi 10h**
- ⬜ Slide roadmap : Blend sur le capital non réservé, parts SEP-41, netting

## Personne 2 — Paiement (couverture + paiement du fournisseur)

**Contrat (`hedge.rs`)** : la marche à suivre est détaillée en tête du fichier.
1. ⬜ `settle` en premier, piloté par les tests : hausse (le pool paie), baisse (Lucía paie sur sa marge), au-delà du plafond (borné), garde-fous (pas avant l'échéance, pas deux fois)
2. ⬜ `open` : `require_auth(owner)`, `pricing::check_cap`, `locked_rate = pricing::forward_rate`, marge = `to_usdc(notional, locked_rate) × cap_bps / BPS`, transfert de la marge, `vault::reserve(marge)`, stockage + `extend_ttl`
3. ⬜ Événements `open` / `settle` (pour le front et l'explorateur)
4. ⬜ Tant que Vault et Pricing ne sont pas prêts : remplacer provisoirement leurs `panic!("TODO")` dans ta branche par des valeurs en dur, **sans les committer**

**Front : écran Lucía (le cœur de la vidéo)**
1. ⬜ Connexion Freighter via Stellar Wallets Kit
2. ⬜ Saisie de la facture : montant, devise, échéance (en secondes pour la démo), adresse du fournisseur
3. ⬜ Affichage du devis (`quote`, fourni par Pricing), choix du plafond, bouton « Bloquer mon taux » → `open_hedge`
4. ⬜ À l'échéance : `settle()` puis **path payment strict receive** USDC → MXN vers le fournisseur, pour exactement le notionnel. Afficher le coût total réel de Lucía = prix figé (aux arrondis et au spread du DEX près)
5. ⬜ Liens vers l'explorateur (stellar.expert testnet) pour les deux transactions
6. ⬜ Une transaction Soroban ne peut pas contenir d'opération classique : ce sont 2 signatures Freighter. À dire honnêtement dans le pitch, avec la version atomique en roadmap

**Hors code**
- ⬜ Script de la démo en 5 temps (voir § Démo)

## Personne 3 — Pricing ⭐ prioritaire

C'est ce qui rend le produit crédible face à un jury finance : un prix faux = arbitrage contre le pool.

**Contrat (`pricing.rs`)**
1. ⬜ `spot` : client SEP-40 sur l'adresse `Oracle`, `lastprice(Asset::Other(currency))`. Refuser si pas de prix ou prix trop ancien (Reflector se met à jour toutes les 300 s). Le même code doit marcher sur Reflector **et** sur le mock
2. ⬜ `forward_rate` : `F = S × (1 + r_USD·t) / (1 + r_dev·t)` en i128 (t = durée / 31 536 000 s), puis le spread **contre l'utilisateur** (BuyCurrency → F × (1 + spread), SellCurrency → F × (1 − spread))
3. ⬜ `quote` : `spot`, `forward_rate`, `usdc_cost = to_usdc(notional, F)`, `margin = usdc_cost × cap_bps / BPS`
4. ⬜ `check_cap` et `set_min_cap` : plafond minimum par devise (ex. MXN 5 %, BRL 7 %, ARS 15 %)
5. ⬜ `set_rate`, `set_spread` (admin, `require_auth`), valeurs initiales réalistes : USD 4 %, MXN 7 %, BRL 10,5 %, ARS (à sourcer)
6. ⬜ Tests du bloc Pricing : test chiffré de la parité (r_MXN 7 %, r_USD 4 %, 90 j → F ≈ 0,05460), sens du spread, plafond minimum
7. ⬜ Ajouter `set_rate` / `set_min_cap` au script `setup-testnet.sh` une fois le contrat prêt

**À traiter (deck et objections)**
- ⬜ Le produit est un **tunnel de change** (collar) symétrique centré sur F : valeur ≈ 0 à l'ouverture, donc le spread suffit à rémunérer le pool. Le justifier en une slide
- ⬜ Calibrer le spread et les plafonds minimum avec la volatilité de chaque devise (un notebook dans `sim/` suffit : P&L de Lucía selon le mouvement du peso, avec et sans protection)
- ⬜ Anti-sélection (on se couvre quand on anticipe un mouvement) → limites par devise et skew en roadmap
- ⬜ Taux d'intérêt en dur = limite assumée du MVP, lecture onchain en roadmap

**Front**
- ⬜ Composant devis (spot réel Reflector, prix figé, marge) utilisé par l'écran de Paiement
- ⬜ Bouton « Crash du peso » : `set_price` sur le mock (compte admin) + script de secours `scripts/crash.sh`
- ⬜ Juste avant la vidéo : aligner le prix du mock et l'offre DEX sur le vrai spot Reflector

---

## Démo (vidéo de 3 min)

1. Lucía et sa facture de 1 000 000 MXN à 90 jours, spot réel lu sur Reflector
2. Plafond 10 %, prix figé, dépôt de marge, signature Freighter
3. « Crash du peso » +12 % : sans protection, la facture coûte ~6 600 $ de plus ; avec, Lucía ne supporte que les 2 % au-delà de son plafond (~1 100 $)
4. Échéance (en secondes) : `settle()` crédite l'USDC, le path payment livre 1 000 000 MXN au fournisseur, visible sur l'explorateur
5. Pool toujours solvable ; slide « suite » : netting, Blend, affacturage, SDK anchors

Deck (8-10 slides) : problème (Lucía) → solution → démo → pourquoi Stellar → protection plafonnée et règle de capacité → pricing → concurrence (Hedgehog) → roadmap → équipe.

## Soumission (lundi 10h-12h, tout le monde)

- ⬜ Merger les 3 branches, redéployer (`./scripts/setup-testnet.sh`), `stellar contract bindings typescript` pour le front
- ⬜ README final : nom, captures/GIF, lien vidéo, IDs à jour
- ⬜ Formulaire HackMeridian : tracks **Stellar Unlocked + Stellar Composed**
- ⬜ Repo accessible au jury

## Hors MVP (slide roadmap, ne pas coder)

Blend · parts LP SEP-41 · netting par devise/échéance · skew de prix · facture transférable / affacturage · SDK anchors · taux d'intérêt onchain · TWAP au fixing · paiement atomique couverture + path payment

## Planning indicatif

| Créneau | Vault | Paiement | Pricing |
|---|---|---|---|
| Dim. soir | deposit / withdraw / reserve + tests | `settle` + tests, wallet front | `spot` + `forward_rate` + tests |
| Nuit / lun. 8h | Panneau pool, trame du deck | `open`, écran Lucía, path payment | `quote`, plafonds, devis front, crash |
| Lun. 8h | **Merge + redéploiement + bindings TS (tous)** | | |
| Lun. 8h-10h | Vidéo | Polish démo | Slides pricing |
| Lun. 10h-12h | Soumission (tous) | | |
