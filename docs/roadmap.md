# Roadmap 4ward : qui fait quoi

Établie le 16/11/2026 à partir des décisions prises à trois les 15 et 16/11. Le détail de chaque lot (changements d'interface, tests, risques) est dans le plan complet, qu'Hugo transmet à part : il n'est pas dans ce repo public. Les efforts sont des estimations en jours ; les dates et l'attribution de `admin.rs` sont des propositions à confirmer lundi.

Chacun travaille sur sa branche. Exception à la règle « on ne touche pas au fichier des autres » : Lancelot écrit trois lots dans `vault.rs`, avec relecture d'Hugo. Rien n'est poussé sur le repo public sans l'accord d'Hugo.

## Calendrier

| Semaine | Objectif | Livrable |
|---|---|---|
| 16 au 22/11 | Spec figée, one-pager envoyé, lots indépendants | Spec d'interface v2 (lundi) ; one-pager chez Leo ; formulaire d'intérêt SCF |
| 23 au 29/11 | Lots dépendants, front | Tous les lots mergés sur `phase-1`, tests verts |
| 30/11 au 6/12 | Intégration, testnet, dossier | Testnet redéployé, parcours complet rejoué, branche et note envoyées à Jia |
| 7 au 20/12 | Skew, front à deux sens, deck, Q&A | Deck investisseur, `docs/qa-investisseurs.md` |
| Janvier 2027 | Dossier d'audit, round SCF, anchor | Dépôt SCF, intégration Etherfuse en bac à sable |

## Lundi 16/11, à trois (une demi-journée)

1. T2 est réglé (16/11) : cours unique à l'échéance ; clôture sans gain ni perte par n'importe qui, une fois l'absence de cours constatée par le contrat.
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
| S47 | Intérêts Safe plafonnés, contrôle du retrait senior, dépôts Safe fermés | B5 | 1,5 |
| S47 | Dernière part junior | B6 | 0,5 |
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
| S47 | Mode démo derrière la feature cargo `demo` | B2 | 0,5 |
| S47 | `checked_mul` dans `hedge::pnl` | B7 (hedge) | 0,5 |
| S48 | `record_fixing` et `settle_fallback`, avec Lancelot pour la lecture de l'oracle | B1 (hedge) | 2 |
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
| S47-S48 | Lecture du cours d'échéance : cours figé, premier cours dans l'heure, constat d'absence | B1 (pricing) | 1 |
| S48 | `admin.rs` : transfert d'admin en deux temps, pause avec clé de garde, upgrade, événements sur les réglages, contrôle des décimales de l'oracle | B10 | 3 |
| S48 | Repris à Hugo, dans `vault.rs` : dépôt refusé tant qu'une couverture échue attend, `pending_settlements` (après B1 et B4) | B3 | 2 |
| S48 | Repris à Hugo, dans `vault.rs` : liste blanche des LP, compteur du notionnel total | B9 (vault) | 1 |
| S48 | Repris à Hugo, dans `vault.rs` : part du protocole côté vault | B11 (vault) | 0,5 |
| S49 | Repris à Hugo : `setup-testnet.sh`, redéploiement, parcours complet rejoué | — | 1,5 |
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
- **Reflector.** Si aucun cours n'est publié le week-end, on n'accepte que des échéances en jours ouvrés ; à savoir avant de coder B1.

## Ce qui reste à décider, sans bloquer la phase 1

Taux de la part du protocole ; montant demandé ; qui dirige et qui vend ; pays de l'entité (après l'avocat) ; date de réouverture du Safe.
