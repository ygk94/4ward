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

- **VERT** : passe et doit continuer.
- **CONSTAT** : passe parce qu'il fige la perte exacte d'un défaut encore présent. À supprimer avec le correctif. Noms commençant par `constat_`.
- **ROUGE** : `#[ignore = "…"]`, échoue à cause d'un défaut connu ; retirer le `#[ignore]` avec le correctif.
- **FUTUR** : dépend d'un lot non écrit. Dans `world.rs`, sous `#[cfg(any())]` (jamais compilé) ; ailleurs, `#[ignore = "FUTUR …"]`. Les noms de fonctions sont tirés des plans, à confirmer.

État au 24/11/2026, après B5 et B6 : `cargo test -p pool` donne 283 réussis, 3 ignorés. Les constats de B5 et B6 ont été supprimés et leurs tests rouges réactivés.

| Test encore ignoré | Pourquoi |
|---|---|
| `attack::deposit_refused_between_maturity_and_settle` | FUTUR, lot B3 (Lancelot). Son constat `constat_deposit_between_maturity_and_settle_captures_half_the_gain` est à supprimer en même temps |
| `platform::ttl_120_day_hedge_entries_live_until_settlement_deadline` | Moitié couverture de B9 (Clément) : durée de conservation alignée sur l'échéance |
| `mutants::senior_served_in_full_after_120_days_saturated` | Décision D1 (`docs/decisions-contrat.md`) : pas de provision d'intérêts dans la capacité pour l'instant |

Constats encore actifs, à supprimer avec leur lot : `platform::transfer_settle_to_frozen_owner_reverts_and_keeps_reserve` (B4), `platform::events_senior_interest_has_no_event_today` (événement d'intérêts senior, non prévu).

## Codes d'erreur supposés

`VaultError` : 116 `LastShareLocked` (B6), 117 `SeniorWithdrawBelowReserved` (B5, garde-fou), 118 `SeniorArrears` (B5), 119 `SeniorDepositsClosed` (B5). Le 113 reste réservé au refus des dépôts de B3 (commentaire de `VaultError` aligné sur la note de Lancelot). Certains fichiers gardent ces numéros en constantes locales.

## Rejouer une suite aléatoire

Un échec de `props.rs` affiche la graine, la suite, le pas, l'état et les dernières opérations, avec la commande exacte, par exemple :

```sh
NET_SEED=0x5eed2026 NET_CHECKS=1f NET_SUITE=1 cargo test -p pool vault::net_props::net_random_sequences_b5 -- --include-ignored --exact --nocapture
```

`NET_SUITES` et `NET_STEPS` allongent une campagne (essai de nuit). La campagne verte par défaut (100 suites × 40 pas) prend environ 17 s en debug.

## Ce que les correctifs font (B5, B6)

- B6 : le plancher porte sur le total des parts junior et se contrôle avant le calcul du montant. D2 : `reserve()` refuse toute couverture (101) tant que le total des parts est sous le plancher. `capacity()` reste la valeur brute, pour que la poussière d'un pool sans couverture puisse toujours sortir ; la lecture peut donc afficher de la capacité alors qu'une ouverture échoue en 101.
- B5 : la créance senior non plafonnée sert au prix d'entrée junior et au ratio de 20 % ; la valeur plafonnée sert aux retraits senior ; la capacité n'est jamais négative.
- Dépôts senior fermés par défaut : chaque aide de test ouvre les dépôts juste après l'enregistrement du pool.
- Tests de `vault::tests` adaptés : deux sorties totales avec une couverture ouverte (désormais jusqu'au plancher), deux tests de capacité négative (désormais 0), et `assert_tranches` (capacité = max(junior − réservé, 0)).
