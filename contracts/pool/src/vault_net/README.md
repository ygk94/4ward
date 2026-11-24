# Filet de tests du vault (phase 1)

Écrit avant les correctifs B5 (intérêts Safe plafonnés) et B6 (plancher de parts junior), pour prouver qu'ils corrigent les défauts sans rien casser. Chaque fichier est un module de test enfant de `vault.rs` (déclaré juste avant `mod tests`) et a ses propres aides.

| Fichier | Angle | Contenu |
|---|---|---|
| `attack.rs` | Attaquant | Scénarios chiffrés au stroop : inflation de part (trois variantes), retrait senior sous plafond, dilution, arrondis, capitalisation répétée du prix senior, pool vidé, dépôt opportuniste avant un règlement |
| `world.rs` | Bout en bout | Uniquement les points d'entrée publics et le vrai mock-oracle : plusieurs devises, pause, liste blanche, plafond de notionnel, cours d'échéance de B1, `close_early` ; tests des lots B1, B3, B4, B11 déjà écrits |
| `platform.rs` | Plateforme Soroban | Signatures exigées (sans `mock_all_auths`), durée de vie du stockage, événements, échec d'un transfert USDC, budget de ressources, unicité des codes d'erreur |
| `mutants.rs` | Tests de mutation | Un test par bug injecté que l'ancienne suite laissait passer, plus les gardes de B5 et B6 |
| `props.rs` | Invariants | `state()` (13 invariants au repos) et moteur de suites d'opérations pseudo-aléatoires par les vrais points d'entrée |

## Statuts

- **VERT** : passe aujourd'hui et doit continuer.
- **CONSTAT** : passe aujourd'hui parce qu'il fige la perte exacte d'un défaut. À supprimer avec le correctif (il deviendra rouge). Noms commençant par `constat_`.
- **ROUGE** : `#[ignore = "ROUGE …"]`, échoue aujourd'hui à cause d'un défaut connu, doit passer après le lot indiqué. Retirer le `#[ignore]` avec le correctif.
- **FUTUR** : dépend d'un lot non écrit. Dans `world.rs`, sous `#[cfg(any())]` (jamais compilé) ; ailleurs, `#[ignore = "FUTUR …"]`. Les noms de fonctions sont tirés des plans, à confirmer.

État au 24/11/2026 : `cargo test -p pool` donne 267 réussis, 22 ignorés ; `cargo test -p pool -- --ignored` : les 22 échouent, comme prévu.

| Lot | Tests ROUGE à réactiver |
|---|---|
| B6 | `attack` : `inflation_*_at_most_one_stroop` (×3) ; `mutants` : `exit_discount_inflation_is_blocked`, `full_exit_with_all_capital_reserved_reports_116`, `leaving_one_stroop_of_shares_with_reserve_reports_116` ; `world` : `r_inflation_by_exit_discount`, `r_last_lp_with_hedge_without_fixing` ; `props` : `net_defect1_*`, `net_random_sequences_b6` |
| B5 | `attack` : `senior_exit_under_cap_is_limited_and_settle_passes`, `half_senior_exit_under_cap_gets_half_of_payable` ; `mutants` : `capacity_never_negative_under_senior_cap`, `senior_exit_under_cap_keeps_losing_settlement_payable`, `senior_served_in_full_after_120_days_saturated` ; `world` : `r_senior_interest_then_withdraw_blocks_losing_settle` ; `platform` : `e2e_defect2_settle_after_full_senior_exit_never_fails` ; `props` : `net_defect2_*`, `net_random_sequences_b5` |
| B5 + B6 | `props` : `net_random_sequences_all` |
| B3 (Lancelot) | `attack` : `deposit_refused_between_maturity_and_settle` |
| B9, côté couverture (Clément) | `platform` : `ttl_120_day_hedge_entries_live_until_settlement_deadline` |

## Codes d'erreur supposés

116 plancher de parts (B6), 117 retrait senior sous le réservé (B5, garde), 118 intérêts senior en retard (B5), 119 dépôts Safe fermés (B5), 113 refus des dépôts tant qu'une couverture échue attend (B3). Ce sont des constantes en tête de chaque fichier : à remplacer par les variantes de `VaultError` une fois écrites. Le 113 est encore ambigu entre la note de Lancelot (B3) et le commentaire de `vault.rs` (retrait senior).

## Rejouer une suite aléatoire

Un échec de `props.rs` affiche la graine, la suite, le pas, l'état et les dernières opérations, avec la commande exacte, par exemple :

```sh
NET_SEED=0x5eed2026 NET_CHECKS=1f NET_SUITE=1 cargo test -p pool vault::net_props::net_random_sequences_b5 -- --include-ignored --exact --nocapture
```

`NET_SUITES` et `NET_STEPS` allongent une campagne (essai de nuit). La campagne verte par défaut (100 suites × 40 pas) prend environ 17 s en debug.

## Ce que le filet suppose des correctifs

- B6 : le plancher porte sur le total des parts junior et se contrôle avant le calcul du montant ; `capacity()` vaut 0 tant que le total des parts est sous le plancher (sinon une couverture peut s'ouvrir sur un pool réduit à la poussière).
- B5 : la créance senior non plafonnée sert au prix d'entrée junior et au ratio de 20 % ; la valeur plafonnée sert aux retraits senior ; la capacité n'est jamais négative.
- Des tests existants de `vault::tests` casseront avec les correctifs : deux avec B6 (sorties jusqu'à la dernière part avec une couverture ouverte), une vingtaine avec la provision d'intérêts de B5 (via `assert_tranches` et `tranched()`).
