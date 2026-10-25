# TODO — HackMeridian

**Deadline : lundi 26/10 à 12h00, sans soumission tardive.** Le jury note de façon asynchrone : **vidéo démo + deck + repo GitHub + README**. Finalistes : pitch live mercredi 28/10.

Légende : ✅ fait · ⬜ à faire · 🔴 bloquant pour la démo

---

## 0. Décisions à prendre maintenant (équipe, 10 min)

- ⬜ 🔴 **Nom du projet** (propositions du council : Pinned, Locktide, Fixa) → remplacer `<NOM>` dans README, front, deck
- ⬜ 🔴 **Qui fait quoi** : répartir les 4 chantiers ci-dessous
- ⬜ **Valider le recadrage du council** : la couverture se termine par le paiement du fournisseur (path payment), pas seulement par un règlement en USDC
- ⬜ **Persona et chiffres de la démo** à figer une fois pour toutes (voir § 5) : Lucía, 1 000 000 MXN, plafond 10 %, peso +12 %

## 1. Setup (par chaque membre)

- ✅ Repo initialisé : workspace Soroban (`contracts/pool`, `contracts/mock-oracle`), front Vite/React/TS, script testnet
- ⬜ Cloner le repo, `rustup target add wasm32v1-none`, `stellar` CLI ≥ 28 (`brew install stellar-cli`)
- ⬜ `stellar contract build && cargo test` passe en local
- ⬜ **Wallet Freighter** (extension navigateur) → Settings → Network : **Testnet**
- ⬜ Importer dans Freighter le compte de démo de Lucía : `stellar keys secret fx-lucia` (sur la machine qui a lancé le setup), ou bien créer son propre compte et le financer via friendbot
- ⬜ Ajouter dans Freighter les actifs de test `USDC` et `MXN` (émetteur `GBRF7DL674464UYOUX5IIXELPGK2OYHVWSLX7GDNR5NO2R6FSGGFQPWL`) pour voir les soldes
- ⬜ Partager les clés `fx-*` entre membres (export `stellar keys secret <nom>` → gestionnaire de mots de passe, **pas dans le repo**)
- ⬜ Pousser le repo sur GitHub (`ygk94/fistea-hackmeridian`) et inviter l'équipe

## 2. Environnement testnet

- ✅ 4 comptes financés : `fx-admin` (émetteur + admin), `fx-lp`, `fx-lucia`, `fx-supplier`
- ✅ Actifs de test émis : USDC et MXN (l'USDC Circle est limité à 20 USDC / 2 h par le faucet, trop peu pour la démo)
- ✅ LP : 1 000 000 USDC · Lucía : 200 000 USDC
- ✅ Offre DEX : 100 M MXN à 0,055 USDC → **path payment USDC→MXN testé** (10 000 MXN reçus par le fournisseur pour 550 USDC)
- ✅ mock-oracle déployé, prix MXN / BRL / ARS chargés au format Reflector (14 décimales)
- ✅ pool déployé (squelette, à redéployer quand le code est écrit : relancer `./scripts/setup-testnet.sh`)
- ⬜ Aligner le prix du mock et de l'offre DEX sur le **vrai spot Reflector** juste avant d'enregistrer la vidéo
- ⬜ Prévoir un script `scripts/crash.sh` : `set_price` MXN +12 % sur le mock (filet de sécurité si le bouton du front plante)

---

## Chantier A — Contrat `pool` (Rust/Soroban) 🔴

Interface figée dans `contracts/pool/src/lib.rs`. Les 4 tests à faire passer sont dans `test.rs` (`#[ignore]`).

1. ⬜ `settle()` en premier, piloté par les tests : hausse (le pool paie), baisse (la PME paie sur sa marge), au-delà du plafond (borné)
2. ⬜ `deposit_lp` / `withdraw` (mapping de soldes, retrait ≤ `capacity()`)
3. ⬜ `quote()` : forward en i128 (pas de flottants) ; spot et F en USD/unité → `F = S × (1 + r_USD·t) / (1 + r_dev·t)`, puis spread
4. ⬜ `open_hedge()` : transfert de la marge (`token::Client::transfer`), contrôle `cap_bps ≥ MinCap`, règle de capacité, stockage de la position
5. ⬜ Lecture de l'oracle via un client SEP-40 (`lastprice`) : même code pour Reflector et le mock, l'adresse est un paramètre
6. ⬜ `extend_ttl` sur les positions et l'instance (une position de 90 jours ne doit pas expirer)
7. ⬜ Événements (`env.events().publish`) sur open / settle, pour le front et l'explorateur
8. ⬜ Redéployer + `stellar contract bindings typescript` → client TS pour le front

Pièges : décimales (USDC 7, oracle 14), arrondis du côté du pool, `require_auth` sur `owner` et `lp`, `settle` permissionless mais une seule fois.

## Chantier B — Front (Vite + React + TS) 🔴

Deps installées : `@stellar/stellar-sdk` 17.4, `@creit.tech/stellar-wallets-kit` 2.7.1. IDs dans `front/.env.local` (généré par le script).

1. ⬜ Connexion Freighter via Stellar Wallets Kit, réseau testnet
2. ⬜ **Écran Lucía** : saisie de la facture (montant, devise MXN/BRL/ARS, échéance), spot réel lu sur Reflector, prix figé (`quote`), choix du plafond, bouton « Bloquer mon taux » (`open_hedge`)
3. ⬜ **Bouton « Crash du peso »** (admin → `set_price` sur le mock) + deux colonnes côte à côte « sans protection / avec protection »
4. ⬜ **Règlement** : `settle()` puis path payment USDC→MXN vers le fournisseur, avec le lien vers l'explorateur (stellar.expert testnet)
5. ⬜ **Panneau pool** minimal : dépôts, capital réservé, capacité restante, solvabilité
6. ⬜ Zéro jargon à l'écran : « protection », « prix figé », pas « NDF », « collar », « SEP-40 »

Commencer avec des données factices tant que le contrat n'est pas prêt, puis brancher les bindings du chantier A.

## Chantier C — Démo, vidéo, deck 🔴

- ⬜ Script de la vidéo (3 min), en 5 temps :
  1. Lucía et sa facture de 1 000 000 MXN à 90 jours, spot réel Reflector
  2. Plafond 10 %, prix figé, dépôt de marge, signature Freighter
  3. « Crash du peso » +12 % : sans protection, la facture coûte ~6 600 $ de plus ; avec, Lucía ne supporte que les 2 % au-delà de son plafond (~1 100 $)
  4. Échéance (en secondes pour la démo) : `settle()` crédite l'USDC, le path payment livre 1 000 000 MXN au fournisseur, visible sur l'explorateur
  5. Pool toujours solvable ; slide « suite » : netting, Blend, affacturage, SDK anchors
- ⬜ Deck (8-10 slides) : problème (Lucía) → solution → démo → pourquoi Stellar → protection plafonnée et règle de capacité → marché → concurrence (Hedgehog) → roadmap → équipe
- ⬜ Réponses prêtes aux objections : plafond vs mouvements de 15 % · marge immobilisée vs banque · anti-sélection · réglementation des dérivés (MVP testnet, KYC via anchors en roadmap)
- ⬜ Enregistrer la vidéo **avant lundi 10h** (marge de 2 h)

## Chantier D — README, soumission, finitions

- ✅ README de spec (à finaliser avec le nom, les captures et le lien vidéo)
- ⬜ Captures d'écran / GIF du parcours dans le README
- ⬜ Formulaire de soumission HackMeridian : tracks **Stellar Unlocked + Stellar Composed**
- ⬜ Vérifier que le repo est public (ou accessible au jury) au moment de la soumission
- ⬜ (optionnel) `sim/` : petit notebook de pricing / risque pour le deck (courbe P&L Lucía vs mouvement du peso)

---

## Hors MVP (slide roadmap, ne pas coder)

Blend · parts LP SEP-41 · netting par devise/échéance · skew de prix · facture transférable / affacturage · SDK anchors · taux d'intérêt onchain · TWAP au fixing

## Planning indicatif (~18 h)

| Créneau | Contrat | Front | Démo / deck |
|---|---|---|---|
| Dim. soir | `settle` + tests, LP | Wallet + écran Lucía (factice) | Nom, script vidéo, trame du deck |
| Nuit / lun. 8h | `quote`, `open_hedge`, déploiement, bindings | Branchement contrat, crash, règlement | Deck |
| Lun. 8h-10h | Gel du code, corrections | Polish, captures | Enregistrement vidéo |
| Lun. 10h-12h | — | — | README final, soumission |
