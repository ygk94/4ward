# Roadmap 4ward : qui fait quoi

Établie le 16/11/2026 à partir des décisions prises à trois les 15 et 16/11. Le détail de chaque lot (changements d'interface, tests, risques) est dans le plan complet, qu'Hugo transmet à part : il n'est pas dans ce repo public. Les efforts sont des estimations en jours ; les dates et l'attribution de `admin.rs` sont des propositions à confirmer lundi.

Chacun travaille sur sa branche. Exception à la règle « on ne touche pas au fichier des autres » : Lancelot écrit trois lots dans `vault.rs`, avec relecture d'Hugo. Rien n'est poussé sur le repo public sans l'accord d'Hugo.

## Engagement Instaward : ce qui prime (ajouté le 25/11)

Le 13/11, on a déposé un dossier Instaward auprès du chapitre Stellar Türkiye : un sprint de 30 jours, qui démarre à la confirmation (pas encore reçue au 25/11). **On sera jugés sur ce dossier**, preuve par preuve. **Il est envoyé et ne peut plus être modifié** : ce qu'il promet est obligatoire, ce n'est plus une option à trancher. La roadmap du 16/11 a été écrite sans lui : cette section liste ce qu'on a promis, ce qui est déjà fait, ce qui contredit le dossier, et ce qui manque. Les lignes concernées plus bas sont marquées **[Instaward]**.

**Objectif à J30 (texte du dossier).** 4ward tourne sur Reflector FX testnet avec un fixing résistant à la manipulation et une voie de secours sûre, règle la couverture et paie au fournisseur le montant exact en MXN dans **une seule transaction atomique**, et a été testé sur testnet par **au moins 5 PME réelles** dont les retours sont publiés.

### Les trois livrables

| Livrable | Promis | Où on en est (25/11) |
|---|---|---|
| **1. Fixing solide** | Redéploiement sur Reflector FX testnet ; fixing = **moyenne pondérée dans le temps sur ~30 min** (TWAP), avec **bornes de cohérence** et contrôle de fraîcheur ; `settle_fallback` ; les correctifs du README (LP qui entre juste avant un règlement gagnant ; intérêts senior qui font passer le junior sous le réservé ou sous 20 % ; contrôle manquant sur `withdraw_senior`) ; `time_factor` derrière une feature de build ; **property tests du fixing et du règlement** | Fait : B5 et B6 (branche vault de Hugo, pas encore dans `main`), cours figé et constat d'absence côté pricing, filet de tests aléatoires du vault. **Contredit le dossier** : le B1 mergé prend un cours unique (voir plus bas). Reste : TWAP et bornes, `settle_fallback`, B3, B2, property tests du fixing et du règlement, redéploiement sur Reflector |
| **2. Régler et payer en une transaction** | Une transaction Soroban : règlement au fixing, puis swap **exact-output** USDC → MXN sur un AMM Soroban (Soroswap ou Aquarius), et le fournisseur reçoit exactement le montant de la facture. La PME signe un **maximum d'USDC** : la transaction échoue plutôt que de payer trop. USDC et MXN de test émis par nous, pool USDC/MXN créé et financé par nous sur le testnet de l'AMM. Bouton « Settle & pay supplier » dans l'app | **Rien** dans le code ni dans la roadmap du 16/11 |
| **3. Pilote** | Sessions guidées sur testnet avec **au moins 5 PME** qui paient des fournisseurs en devise (importateurs en Europe ou en Amérique latine). Accueil dans l'app : explication du plafond et de la marge en langage simple, saisie de la facture, confirmation de couverture téléchargeable. **Rapport de pilote public** : qui a testé, ce qu'ils ont couvert, ce qui les a bloqués, devises et échéances voulues | **Rien** de planifié |

### Ce qui contredit le dossier

- **Fixing en cours unique (décision T2 du 16/11, B1 mergé le 24/11).** Le dossier promet un TWAP sur ~30 min avec bornes ; le code prend un seul cours (à l'échéance, sinon le premier publié dans l'heure). Rien n'est à jeter : fenêtre de grâce, cours figé (`record_fixing`) et constat d'absence se gardent. Il faut remplacer la lecture d'un cours par une moyenne (proposition : 6 cours espacés de 300 s qui finissent à l'échéance, lus dans l'historique de Reflector) et refuser un cours trop loin de cette moyenne. **Obligatoire** : le dossier ne peut plus être modifié, le cours unique doit être remplacé par le TWAP avec bornes (Lancelot, B1 pricing).
- **Solde réclamable (B4) seul.** Le dossier promet que le fournisseur est payé dans la transaction du règlement. B4 n'est pas faux (il protège le pool si le versement échoue), mais il ne suffit pas : il faut ajouter `settle_and_pay`, qui règle puis consomme le versement de la PME dans le swap. **`settle_and_pay` est obligatoire.** Seule question ouverte : garder aussi `settle` + `claim` à côté (pour le robot et les cas où le swap échoue), ou non.

### Hors périmètre du dossier

Mainnet et vrais fonds, audit externe, réglementaire et KYC, vraie liquidité LP ou AMM, paiement atomique dans une autre devise que le MXN, netting et couverture du pool, placement en bons du Trésor, appli mobile. Les lots déjà faits pour le mainnet (B10 : pause, transfert d'admin, mise à jour du code) ne sont pas faux, mais **pendant les 30 jours, les livrables du dossier passent avant** les lots de la phase 1 qui n'y figurent pas (B9 PME, B11, skew, front à deux sens, deck).

### Calendrier du sprint (semaine 1 = semaine de la confirmation)

| Semaine | Travail (dossier) | Preuve attendue | Qui (proposition) |
|---|---|---|---|
| S1 | TWAP avec bornes et fraîcheur, et ses property tests ; `time_factor` derrière une feature `demo` ; début du recrutement des PME | Fixing codé et testé sur une branche ; PME contactées | Lancelot (TWAP), Clément (B2, tests du règlement), Hugo (recrutement) |
| S2 | `settle_fallback` et les correctifs (B3, B5, B6) ; redéploiement sur **Reflector FX testnet** ; une couverture **d'un jour en mode production** (sans accélération), réglée par le keeper peu après l'échéance ; création et financement du pool USDC/MXN sur l'AMM | Pool v1.1 en ligne (lien stellar.expert), notes de version, hash de la couverture d'un jour réglée au fixing Reflector, pool AMM financé | Clément (`settle_fallback`, keeper), Lancelot (redéploiement, B3), Hugo (B5, B6), Clément ou Lancelot (pool AMM) |
| S3 | `settle_and_pay` dans le contrat et le front ; améliorations de l'accueil ; premières sessions pilote | Transaction d'exemple sur testnet ; app à jour ; premiers retours | Clément (contrat), Hugo (front, accueil) |
| S4 | Sessions restantes, corrections, deux vidéos, rapport de pilote, documentation finale | 5 PME ou plus ; rapport public ; clip « Settle & pay » (1 min) et vidéo du sprint (3 min) ; preuves envoyées à la responsable du chapitre | Les trois |

### Preuves à envoyer à la fin

- **Livrable 1** : release GitHub taguée dont les notes relient chaque correctif à son test ; le contrat sur stellar.expert, branché sur l'adresse Reflector FX testnet ; les hash d'ouverture et de règlement d'une couverture d'un jour en mode production, réglée au TWAP Reflector ; une page de CI avec tous les tests verts (la CI ne lance aujourd'hui que le build du front : y ajouter `cargo test`).
- **Livrable 2** : une transaction stellar.expert où la couverture est réglée, l'USDC échangé sur l'AMM, et le fournisseur reçoit exactement 1 000 000 MXN de test ; un clip d'une minute sur 4ward-fx.vercel.app.
- **Livrable 3** : rapport listant les 5 PME ou plus (anonymisées si elles le préfèrent), leurs couvertures (liens testnet), leurs retours et nos conclusions ; captures avant/après de l'accueil ; vidéo de 3 min du parcours complet.

## Calendrier

| Semaine | Objectif | Livrable |
|---|---|---|
| 16 au 22/11 | Spec figée, one-pager envoyé, lots indépendants | Spec d'interface v2 (lundi) ; one-pager chez Leo ; formulaire d'intérêt SCF |
| 23 au 29/11 | Lots dépendants, front | Tous les lots mergés sur `phase-1`, tests verts |
| 30/11 au 6/12 | Intégration, testnet, dossier | Testnet redéployé, parcours complet rejoué, branche et note envoyées à Jia |
| 7 au 20/12 | Skew, front à deux sens, deck, Q&A | Deck investisseur, `docs/qa-investisseurs.md` |
| Janvier 2027 | Dossier d'audit, round SCF, anchor | Dépôt SCF, intégration Etherfuse en bac à sable |

## Lundi 16/11, à trois (une demi-journée)

1. T2 est réglé (16/11) : cours unique à l'échéance ; clôture sans gain ni perte par n'importe qui, une fois l'absence de cours constatée par le contrat. **[Instaward]** Le cours unique contredit le dossier (TWAP sur ~30 min avec bornes), qui est envoyé et ne peut plus changer : le TWAP remplace le cours unique. La clôture sans gain ni perte après constat d'absence reste valable (c'est le `settle_fallback` promis).
2. Confirmer qui écrit `admin.rs` (proposition : Lancelot, le moins chargé côté contrat).
3. Figer la nouvelle interface de `lib.rs` : `open_hedge` avec `worst_rate`, `record_fixing`, `settle_fallback`, `claim`, `claim_for`, `claimable`, `pending_settlements`, `pilot_limits`, `is_allowed`, `paused`, fonctions admin.
4. Répartir les numéros d'erreur : Vault 113 à 116, Paiement 207 à 215, Pricing 313, Admin 400 et suivants.

## Hugo : vault, front, dossiers

Moins de contrat, plus de transversal : Lancelot reprend le refus des dépôts, les parties vault des limites et de la part du protocole, et le redéploiement. Hugo reste relecteur de tout ce qui touche `vault.rs`.

| Quand | Tâche | Lot | Effort |
|---|---|---|---|
| S47 | One-pager pour Leo, relu à trois | A | 1 |
| S47 | Formulaire d'intérêt SCF avec un parrain (contact : Gildas Soeiro) | E | 0,5 |
| S47 | Vérifier que la démo publique tourne encore (flux peso de Reflector testnet expiré depuis le 09/11) | E | 0,5 |
| S47 | Intérêts Safe plafonnés, contrôle du retrait senior, dépôts Safe fermés **[Instaward, livrable 1]** | B5 | 1,5 |
| S47 | Dernière part junior | B6 | 0,5 |
| Sprint S1-S4 | **[Instaward]** Recrutement et sessions guidées des 5 PME du pilote ; accueil dans l'app (plafond et marge expliqués simplement, saisie de la facture, confirmation téléchargeable) ; bouton « Settle & pay supplier » ; rapport de pilote public ; clip « Settle & pay » et vidéo du sprint | Livrables 2 et 3 | à estimer |
| S48-S49 | Front : `open_hedge` avec taux limite, âge du prix, solde réclamable, limites du pilote, règlement avant dépôt, bandeau de pause, table d'erreurs | Front | 4 |
| S49 | Dossier pour l'audit, partie écrite : threat model, diagramme de flux, liste des invariants ; note de changements pour Jia ; README et docs | B12 | 2 |
| S49 | Envoi de la branche à Jia | — | — |
| S50-S51 | Front à deux sens : saisie en dollars, conversion pour soi-même, choix de la devise | 1 bis | 5 |
| S50-S51 | Deck investisseur | C | 3 |
| En continu | Consultation d'avocat, SCF, relation 57 Blockchain, 3 entretiens de PME | E | — |

## Clément : paiement (`hedge.rs`)

| Quand | Tâche | Lot | Effort |
|---|---|---|---|
| S47 | Solde réclamable : `claim`, `claim_for`, `claimable` ; `settle` crédite au lieu de transférer | B4 | 2,5 |
| S47 | Mode démo derrière la feature cargo `demo` **[Instaward, livrable 1]** | B2 | 0,5 |
| S47 | `checked_mul` dans `hedge::pnl` | B7 (hedge) | 0,5 |
| S48 | `record_fixing` et `settle_fallback`, avec Lancelot pour la lecture de l'oracle **[Instaward, livrable 1]** | B1 (hedge) | 2 |
| Sprint S1-S2 | **[Instaward]** Property tests du règlement (P&L borné, versement entre 0 et 2 × marge, conservation de l'USDC du contrat) | Livrable 1 | 1 |
| Sprint S2 | **[Instaward]** Couverture d'un jour en mode production sur Reflector, réglée par le keeper peu après l'échéance (hash pour les preuves) | Livrable 1 | 0,5 |
| Sprint S3 | **[Instaward]** `settle_and_pay(hedge_id, max_usdc)` : règlement, puis swap exact-output USDC → MXN sur un AMM Soroban (Soroswap ou Aquarius), fournisseur payé du montant exact, échec au-delà de `max_usdc` | Livrable 2 | 3 à 4 |
| S48 | `worst_rate` sur `open_hedge` | B8 | 1 |
| S48 | Liste blanche des PME, plafonds par couverture et de durée, nombre maximal de couvertures ouvertes | B9 (hedge) | 1,5 |
| S48 | Part du protocole, prélevée au règlement | B11 (hedge) | 1 |
| S49 | `scripts/keeper.sh` (appel de `record_fixing`, `settle`, `claim_for`), `scripts/seed-testnet.sh`, `docs/payment-cli.md` (table d'erreurs) | — | 1,5 |
| S50-S51 | Préparation du Q&A (`docs/qa-investisseurs.md`) | D | 2 |
| En continu | 3 entretiens de PME ; premier contact Bitso Business | E | — |

## Lancelot : pricing (`pricing.rs`), `admin.rs`, et les lots repris à Hugo

| Quand | Tâche | Lot | Effort |
|---|---|---|---|
| S47 | Vérifier Reflector sur mainnet : cours le week-end, prolongation du flux peso (expire le 21/11), coût | E | 0,5 |
| S47 | `checked_mul` dans `to_usdc` et `mul_div`, code 313 | B7 (pricing) | 0,5 |
| S47 | Âge maximal du prix à l'ouverture ramené à 600 s | B8 (pricing) | 0,5 |
| S47-S48 | Lecture du cours d'échéance : cours figé, premier cours dans l'heure, constat d'absence (fait, mergé le 24/11) | B1 (pricing) | 1 |
| Sprint S1 | **[Instaward]** Fixing en TWAP sur ~30 min (moyenne de cours de l'historique Reflector) avec bornes de cohérence et contrôle de fraîcheur, à la place du cours unique ; property tests du fixing | Livrable 1 | 2 |
| S48 | `admin.rs` : transfert d'admin en deux temps, pause avec clé de garde, upgrade, événements sur les réglages, contrôle des décimales de l'oracle (fait ; hors périmètre Instaward, sert au mainnet) | B10 | 3 |
| S48 | Repris à Hugo, dans `vault.rs` : dépôt refusé tant qu'une couverture échue attend, `pending_settlements` (après B1 et B4) **[Instaward, livrable 1]** | B3 | 2 |
| S48 | Repris à Hugo, dans `vault.rs` : liste blanche des LP, compteur du notionnel total | B9 (vault) | 1 |
| S48 | Repris à Hugo, dans `vault.rs` : part du protocole côté vault | B11 (vault) | 0,5 |
| S49 | Repris à Hugo : `setup-testnet.sh`, redéploiement, parcours complet rejoué. **[Instaward]** Le pool doit être branché sur **Reflector FX testnet**, plus sur le mock-oracle ; release taguée et `cargo test` dans la CI | — | 1,5 |
| Sprint S2 | **[Instaward]** Pool USDC/MXN de test créé et financé sur l'AMM Soroban choisi (avec Clément) | Livrable 2 | 1 |
| S49 | Dossier pour l'audit, partie outillée : rapport Scout, fuzzing des fonctions de calcul | B12 | 1 |
| S50-S51 | Skew de prix (0,25 à 1 %), calibré avec `sim/` | 1 bis | 3 |
| S50-S51 | Slide « économie du pool et risque » ; bac à sable Etherfuse : un devis USDC vers MXN réel | C, E | 1,5 |
| En continu | 3 entretiens de PME | E | — |

## Charge de la phase 1 (jusqu'à l'envoi à Jia)

| | Contrat | Front, scripts, dossiers | Total |
|---|---|---|---|
| Hugo | 2 | 8 (front, one-pager, SCF, dossier écrit) | 10 |
| Clément | 9 | 1,5 | 10,5 |
| Lancelot | 8,5 | 3 (Reflector, redéploiement, outils d'audit) | 11,5 |

Attribution des modules d'après le `TODO.md` et le deck : Clément au paiement, Lancelot au pricing. **À confirmer** : si c'est l'inverse, les sections Clément et Lancelot s'échangent.

## Points de dépendance

- **B4 avant B1 avant B3.** Clément livre le solde réclamable, puis le cours figé ; Lancelot branche ensuite le refus des dépôts dans `vault.rs`, relu par Hugo.
- **Le front attend la spec de lundi**, pas la fin des lots : Hugo peut coder contre l'interface figée.
- **Bascule du testnet.** `open_hedge` change de signature : pool et front sont redéployés ensemble.
- **[Instaward] TWAP et voie de secours.** `record_fixing` et `settle_fallback` (faits par Clément le 25/11) ne dépendent pas de la façon dont le cours est lu : le TWAP se branche dans la lecture du pricing sans les changer. Les tests du règlement devront être relancés une fois le TWAP en place.
- **[Instaward] B4 et `settle_and_pay`.** Le versement de la PME doit pouvoir être consommé par le swap dans la même transaction : à prévoir dans la forme du solde réclamable.
- **[Instaward] Pool AMM avant `settle_and_pay`.** Sans pool USDC/MXN financé sur le testnet de l'AMM, rien à tester en S3.
- **Reflector.** Si aucun cours n'est publié le week-end, on n'accepte que des échéances en jours ouvrés ; à savoir avant de coder B1.

## Ce qui reste à décider, sans bloquer la phase 1

Taux de la part du protocole ; montant demandé ; qui dirige et qui vend ; pays de l'entité (après l'avocat) ; date de réouverture du Safe.

**[Instaward] Le dossier est envoyé et ne peut plus être modifié** : TWAP avec bornes, `settle_and_pay` atomique et pilote avec 5 PME sont obligatoires. **Reste à trancher à trois :** garder `settle` + `claim` à côté de `settle_and_pay` ou non ; quel AMM (Soroswap ou Aquarius) ; qui recrute les 5 PME, et quand ; date du sprint une fois la confirmation reçue.
