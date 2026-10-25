#![cfg(test)]
//! Tests de départ, un bloc par personne : retirer les #[ignore] au fur et à mesure.
//! Scénario commun : 1 000 000 MXN, spot 0.055 USD/MXN, plafond 10 %, oracle = mock-oracle.

// ---- Vault ----

#[test]
#[ignore = "TODO vault: dépôt puis retrait, soldes et TotalDeposits à jour"]
fn vault_deposit_withdraw() {}

#[test]
#[ignore = "TODO vault: retrait refusé au-delà du capital non réservé"]
fn vault_withdraw_limited_to_capacity() {}

// reserve / release / book_gain / book_loss : tests dans vault.rs (mod tests).

// ---- Pricing ----

#[test]
#[ignore = "TODO pricing: r_MXN 7 %, r_USD 4 %, 90 j → F ≈ 0.05460 avant spread"]
fn pricing_forward_parity() {}

#[test]
#[ignore = "TODO pricing: le spread est contre l'utilisateur dans les deux sens"]
fn pricing_spread_direction() {}

#[test]
#[ignore = "TODO pricing: plafond sous le minimum de la devise refusé"]
fn pricing_min_cap() {}

// ---- Paiement ----

#[test]
#[ignore = "TODO paiement: la devise monte de 8 %, le pool paie l'écart à l'importateur"]
fn settle_currency_up_pool_pays() {}

#[test]
#[ignore = "TODO paiement: la devise baisse de 8 %, l'importateur paie le pool sur sa marge"]
fn settle_currency_down_user_pays() {}

#[test]
#[ignore = "TODO paiement: la devise monte de 15 %, paiement borné au plafond de 10 %"]
fn settle_beyond_cap_is_capped() {}

#[test]
#[ignore = "TODO paiement: settle refusé avant l'échéance et refusé deux fois"]
fn settle_guards() {}
