// [FRONT — Hugo] IDs testnet, générés par ../scripts/setup-testnet.sh dans .env.local
export const config = {
  rpcUrl: import.meta.env.VITE_RPC_URL as string,
  networkPassphrase: import.meta.env.VITE_NETWORK_PASSPHRASE as string,
  issuer: import.meta.env.VITE_ISSUER as string,
  usdcSac: import.meta.env.VITE_USDC_SAC as string,
  mxnSac: import.meta.env.VITE_MXN_SAC as string,
  poolId: import.meta.env.VITE_POOL_ID as string,
  mockOracleId: import.meta.env.VITE_MOCK_ORACLE_ID as string,
  reflectorFxId: import.meta.env.VITE_REFLECTOR_FX_ID as string,
  supplier: import.meta.env.VITE_SUPPLIER as string,
}

/** Objectif de collecte du vault (USDC) : la jauge de la page d'accueil se remplit en proportion. */
export const VAULT_TARGET_USDC = Number(import.meta.env.VITE_VAULT_TARGET_USDC ?? 2_000_000)
/** Nom du projet (logo : public/logo-4ward.svg, favicon : public/favicon.svg). */
export const BRAND = '4ward'
/** Devises ouvertes à la cotation sur le pool (set_min_cap dans scripts/setup-testnet.sh). */
export const TRADABLE = ['MXN', 'BRL', 'ARS', 'EUR']
