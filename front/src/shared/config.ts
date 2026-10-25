// [VAULT — partagé] IDs testnet, générés par ../scripts/setup-testnet.sh dans .env.local
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
