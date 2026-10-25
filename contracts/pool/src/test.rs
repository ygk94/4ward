#![cfg(test)]
//! Première tâche du chantier contrat : faire passer ces tests (retirer les #[ignore]).
//! Scénario commun : 1 000 000 MXN, taux bloqué 0.055 USD/MXN, plafond 10 %, oracle = mock-oracle.

#[test]
#[ignore = "TODO: settle — la devise monte de 8 %, le pool paie l'écart à l'importateur"]
fn settle_currency_up_pool_pays() {}

#[test]
#[ignore = "TODO: settle — la devise baisse de 8 %, l'importateur paie le pool sur sa marge"]
fn settle_currency_down_user_pays() {}

#[test]
#[ignore = "TODO: settle — la devise monte de 15 %, paiement borné au plafond de 10 %"]
fn settle_beyond_cap_is_capped() {}

#[test]
#[ignore = "TODO: open_hedge refuse si la capacité du pool est dépassée"]
fn open_hedge_rejects_over_capacity() {}
