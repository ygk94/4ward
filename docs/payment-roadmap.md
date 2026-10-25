# Roadmap — Paiement

Périmètre : `contracts/pool/src/hedge.rs`, les tests Paiement de `test.rs`, le parcours testnet en ligne de commande, le déroulé des appels pour le front d'Hugo, le script vidéo.
Branche : `feat/payment` → PR vers `main` avant le merge d'Hugo.

Légende : ✅ fait · ⬜ à faire · 🔴 bloque quelqu'un d'autre

---

## 0. Prérequis ✅

- ✅ Rust 1.99, cible `wasm32v1-none`, Stellar CLI 28.1, rust-analyzer
- ✅ `cargo test` et `stellar contract build` passent
- ✅ Branche `feat/payment`

## 1. Banc de test

Objectif : pouvoir tester `settle` alors que Vault et Pricing ne sont pas codés.

- ✅ Extraire le calcul du P&L dans une **fonction pure** de `hedge.rs` :
  `fn pnl(side, notional, locked_rate, spot, margin) -> i128` (borné à ±marge).
  Testable sans oracle, sans token, sans Vault : c'est le cœur des 3 tests hausse / baisse / plafond.
- ✅ Fixture commune dans `test.rs` : `Env`, admin, USDC (`register_stellar_asset_contract_v2`), mock-oracle, pool avec son constructeur, Lucía, fournisseur, LP.
- ✅ 🔴 Ajouter `mock-oracle` en `dev-dependencies` de `contracts/pool/Cargo.toml` (`path = "../mock-oracle"`). **Fichier partagé : prévenir Hugo et Pricing.**
- ✅ (à l'étape 2, quand `settle` en a besoin) Stubs locaux de Vault et Pricing (valeurs en dur) **jamais committés** : on committe fichier par fichier (`git add contracts/pool/src/hedge.rs …`), jamais `git add -A`.

## 2. `settle` (TDD, en premier)

Ordre : écrire le test → le voir échouer → coder → le voir passer.

- ✅ Test `settle_currency_up_pool_pays` : MXN +8 % → P&L = +4 530 USDC, Lucía reçoit marge + 4 530
- ✅ Test `settle_currency_down_user_pays` : MXN −8 % → P&L = −4 270, Lucía reçoit marge − 4 270
- ✅ Test `settle_beyond_cap_is_capped` : MXN +15 % → P&L borné à +marge
- ✅ Test `settle_guards` : refus avant l'échéance, refus d'un 2ᵉ `settle`, refus d'un id inconnu
- ✅ Test en plus : `SellCurrency` (exportateur), signe inversé
- ✅ Test en plus : `settle_total_loss_pays_nothing` (−15 %, rien à rendre)
- ✅ Tests `settle` marqués `#[ignore = "attend Vault + Pricing"]` : en local `cargo test -- --include-ignored`, à dé-ignorer après le merge
- ✅ Code de `settle` :
  1. charger la couverture (panic si absente), refuser si `settled` ou `timestamp < maturity`
  2. `spot = pricing::spot`, `pnl = pnl(...)`
  3. `vault::release(margin)`, puis `book_loss(pnl)` si pnl > 0, `book_gain(-pnl)` si pnl < 0
  4. transfert USDC contrat → owner de `margin + pnl` (sauter si 0)
  5. `settled = true`, sauvegarde + `extend_ttl`, retour du montant versé

Règles de calcul : tout en `i128`, **arrondis en faveur du pool** (P&L payé par le pool arrondi vers le bas, P&L encaissé arrondi vers le haut). Ordres de grandeur : notionnel 10¹³ × prix 10¹² = 10²⁵, bien sous la limite i128 (~1,7 × 10³⁸).

## 3. Script vidéo (3 min, 5 temps) 🔴

- ✅ (`docs/video-script.md`) Rédiger les 5 temps du § Démo du TODO : texte dit, écran montré, action, durée
- ✅ Chiffres exacts de la démo (facture, prix figé, marge, crash +12 %, coût avec et sans protection)
- ✅ Phrase honnête sur les 2 signatures (`settle` puis path payment, atomicité en roadmap)
- ⬜ Livrer à Hugo

## 4. `open`

- ✅ Tests : marge = `to_usdc(notional, locked_rate) × cap_bps / BPS` (arrondie vers le haut), USDC bien transférés, `Reserved` augmenté, couverture lisible via `get_hedge`, ids qui s'incrémentent
- ✅ Tests de refus : plafond sous le minimum, capacité du pool insuffisante, notionnel ou durée ≤ 0, sans signature du owner
- ✅ Code : `require_auth(owner)`, `check_cap`, `forward_rate`, calcul de marge, transfert owner → contrat, `vault::reserve`, `NextId`, stockage persistant + `extend_ttl`, retour de l'id
- ✅ Test bout en bout `open_then_settle_full_flow` (ouverture puis règlement à +8 %)
- ✅ Branché sur le vrai Vault (`main`) : `vault::open_position(&hedge)` dans `open`, `vault::close_position(&hedge)` dans `settle` (exposition par devise, limite par devise #108)
- ✅ Codes d'erreur `HedgeError` 200+ (`UnknownHedge`, `AlreadySettled`, `NotMatured`, `InvalidNotional`, `InvalidDuration`) : les textes de `panic!` sont effacés dans le WASM, le front a besoin des codes
- ✅ Fixtures de test sur les vrais appels (`deposit_lp`, `open_hedge`, `total_assets`, `capacity`) ; tests marqués `#[ignore = "attend Pricing"]`
- ✅ Refus sans la signature du owner : `open_refused_without_owner_signature` (signé par le fournisseur ou par personne → refusé, rien ne bouge ; témoin signé par Lucía → passe ; le test casse si on retire `require_auth`) et `settle_needs_no_signature`

## 5. Événements

- ✅ `hedge_opened` (topics : id, owner) : fournisseur, devise, sens, notionnel, taux bloqué, marge, échéance
- ✅ `hedge_settled` (topics : id, owner) : spot de fixing, P&L, montant versé
- ✅ Tests `open_emits_event` / `settle_emits_event` (comparaison exacte), présents dans le spec du contrat (`#[contractevent]`, donc dans les bindings TS)

## 6. PR vers `main` 🔴

- ⬜ Retirer les `#[ignore]` des tests Paiement
- ⬜ Aucun stub Vault / Pricing dans le diff (`git diff main --stat` ne doit montrer que `hedge.rs`, `test.rs`, `Cargo.toml` du pool, `docs/`)
- ⬜ `cargo test` + `stellar contract build` OK, CI verte
- ⬜ Prévenir Hugo : les tests Paiement ne passeront qu'une fois Vault et Pricing fusionnés

## 7. Parcours CLI testnet 🔴 pour le front d'Hugo

- ✅ Montrer à Clément comment suivre chaque transaction : hash dans la CLI, page stellar.expert (opérations, événements, appels de contrat), soldes des comptes avant / après
- ✅ Répétition sur un pool à moi : les 6 transactions validées (dépôt LP, `open_hedge`, crash +8 %, `settle` signé par le fournisseur, réalignement DEX, path payment de 1 000 000 MXN) ; coût total de Lucía = 54 870 USDC (déployé depuis ma branche avec stubs) si le merge tarde
- ✅ (`docs/payment-cli.md`) Commandes exactes, dans l'ordre : `deposit_lp` (fx-lp) → `quote` → `open_hedge` (fx-lucia, échéance ~60 s) → crash du mock → `settle` → `stellar tx new path-payment-strict-receive` USDC → MXN vers fx-supplier pour 1 000 000 MXN pile
- ✅ Pour Hugo, le déroulé des appels côté front :
  - montants à afficher : prix figé, marge déposée, P&L, montant versé par `settle`, coût total réel (= prix figé dans le plafond)
  - cas d'erreur : échéance pas atteinte, déjà réglée, capacité insuffisante, plafond trop bas, prix oracle trop ancien, path payment sans liquidité (`op_too_few_offers`) ou au-dessus du `send-max`
  - liens stellar.expert des 2 transactions

## 8. DEX MXN/USDC

- ✅ Vérifier la profondeur de l'offre (100 000 000 MXN à 0,055) pour 1 000 000 MXN
- ✅ Réalignement testé : `manage-sell-offer --offer-id <id> --price "297:5000"` (0,0594)
- ✅ 🔴 Après le crash +12 %, **réaligner l'offre sur le nouveau prix du mock** (commande pour Hugo dans `docs/payment-cli.md` § 5, au lieu d'un script), sinon le coût total de Lucía ne tombe plus sur le prix figé. Script `scripts/align-dex.sh` (nouveau fichier, le mien)

## 9. Après le merge

- ✅ `main` (Vault phases 5-6, Pricing de Lancelot) fusionné dans `feat/payment`, stubs supprimés
- ✅ `#[ignore]` retirés : 111/111 tests sur le vrai Vault et le vrai Pricing (fixture : `set_rate` USD 4 % / MXN 7 %, `set_min_cap` MXN 5 %)
- ✅ Tests `open_*` / `settle_*` recalés sur le vrai taux bloqué (0,0552750 à 60 s) ; tests d'événements adaptés aux événements du vault
- ✅ `open` utilise `pricing::margin_for` (marge identique à celle de `quote`, demandé par Pricing)
- ✅ Chiffres du script vidéo et de `payment-cli.md` recalculés avec le vrai Pricing (spot 0,055)
- ✅ Parcours rejoué sur testnet avec le code fusionné (mon déploiement, commandes de `docs/payment-cli.md` telles quelles) : coût total 56 072,50 au centime, codes #200 / #201 / #202 vus sur le réseau
- ⬜ Rejouer le parcours sur le déploiement d'Hugo (commandes `docs/payment-cli.md`)
- ⬜ Vérifier les chiffres affichés par le front contre ceux de la CLI


## 10. Points à trancher (`docs/points-a-trancher.md`)

- ✅ Point 2, durée de démo (option b) : `set_time_factor` / `time_factor` (admin, 1 par défaut, #205 si 0, événement `time_factor_set`) ; prix sur la durée annoncée, échéance réelle = durée / facteur (au moins 1 s). Démo : 25 920 → 90 jours en 300 s ; chiffres du script et de `payment-cli.md` passés à 90 jours
- ⬜ Point 1, fixing à l'échéance : `settle` appelle `pricing::fixing(env, &hedge.currency, hedge.maturity)` au lieu de `spot` (interface de Lancelot, erreur 312 `NoFixing`) ; tests avec l'historique du mock-oracle ; à faire dès que Lancelot a poussé sur `feat/pricing`

---

## À caler avec les autres

| Avec | Sujet |
|---|---|
| Pricing + Hugo | `spot` refuse un prix de plus de 300 s, et `forward_rate` le lit : le prix du mock doit être remis à jour juste avant **`open_hedge`** et juste avant **`settle`** dans la démo (vu sur testnet : prix posé 765 s avant l'ouverture) |
| Pricing | Erreurs de Pricing en `#[contracterror]` (codes 300+ ?) pour que le front distingue « plafond trop bas » / « prix trop ancien » |
| Pricing | Formule du forward : le README a raison (`r_USD / r_devise`), le transcript du council est inversé |
| Pricing | Prix du mock après crash → je réaligne l'offre DEX |
| Vault | ✅ Réglé : `open_position` / `close_position`, transfert USDC vers la PME fait par `hedge.rs` |
| Vault + Pricing | `mock-oracle` ajouté en dev-dependency du pool |
| Hugo | Script vidéo, commandes CLI, déroulé des appels, PR |
