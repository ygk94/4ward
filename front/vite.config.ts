import react from '@vitejs/plugin-react'
import { defineConfig, loadEnv } from 'vite'

// Variables lues par src/shared/config.ts : sans elles le site se charge mais reste vide.
// Le build échoue s'il en manque une (cas d'un déploiement Vercel où on a oublié de les renseigner).
const REQUIRED = [
  'VITE_RPC_URL',
  'VITE_NETWORK_PASSPHRASE',
  'VITE_ISSUER',
  'VITE_USDC_SAC',
  'VITE_MXN_SAC',
  'VITE_POOL_ID',
  'VITE_MOCK_ORACLE_ID',
  'VITE_REFLECTOR_FX_ID',
  'VITE_SUPPLIER',
]

// https://vite.dev/config/
export default defineConfig(({ command, mode }) => {
  if (command === 'build') {
    const env = loadEnv(mode, process.cwd(), 'VITE_')
    const missing = REQUIRED.filter((k) => !env[k])
    if (missing.length) {
      throw new Error(`Variables d'environnement manquantes : ${missing.join(', ')} (voir front/.env.example)`)
    }
  }
  return { plugins: [react()] }
})
