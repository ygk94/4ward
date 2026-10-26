// [FRONT — Hugo] Taux Etherfuse en direct (API publique, sans clé ; CORS ouvert à 4ward-fx.vercel.app) et taux
// réellement utilisé par le pool, retrouvé depuis un devis (le contrat n'a pas de getter de taux).
// Le contrat ne lit pas Etherfuse : l'admin pousse ces taux avec scripts/sync-rates.sh (set_rate).
import { useEffect, useState } from 'react'
import type { Quote, SideTag } from './contract'

export const ETHERFUSE_URL = 'https://api.etherfuse.com/lookup/tokens/cost'
/** Page publique des stablebonds, pour le lien « source ». */
export const ETHERFUSE_SITE = 'https://www.etherfuse.com/'

/** Devise → stablebond Etherfuse, même correspondance que scripts/sync-rates.sh (null : taux fixe, ARS). */
export const RATE_TOKEN: Record<string, string | null> = { USD: 'USTRY', MXN: 'CETES', BRL: 'TESOURO', EUR: 'EUROB', ARS: null }

/** Spread du pool en bps : SPREAD_BPS de scripts/setup-testnet.sh (pas de getter onchain). */
const POOL_SPREAD_BPS = 50
const SECS_PER_YEAR = 31_536_000
const REFRESH_MS = 5 * 60_000

export interface EtherfuseRates {
  /** Taux annuel en bps par stablebond (current_basis_points). */
  bps: Record<string, number>
  updatedAt: Date
}

async function fetchRates(): Promise<EtherfuseRates> {
  const res = await fetch(ETHERFUSE_URL)
  if (!res.ok) throw new Error(`Etherfuse ${res.status}`)
  const data = (await res.json()) as Record<string, { current_basis_points?: unknown }>
  const bps: Record<string, number> = {}
  for (const sym of Object.values(RATE_TOKEN)) {
    const v = sym ? data[sym]?.current_basis_points : undefined
    if (sym && typeof v === 'number' && Number.isInteger(v)) bps[sym] = v
  }
  if (!bps.USTRY) throw new Error('Etherfuse : USTRY absent')
  return { bps, updatedAt: new Date() }
}

/**
 * Taux Etherfuse relus toutes les 5 min ; null tant qu'aucune lecture n'a réussi (la ligne disparaît, rien ne casse).
 * Un rafraîchissement en échec garde la lecture précédente, avec son heure : l'API a des timeouts passagers.
 */
export function useEtherfuseRates(): EtherfuseRates | null {
  const [rates, setRates] = useState<EtherfuseRates | null>(null)
  useEffect(() => {
    let alive = true
    const load = () =>
      fetchRates()
        .then((r) => alive && setRates(r))
        .catch(() => undefined)
    load()
    const id = setInterval(load, REFRESH_MS)
    return () => {
      alive = false
      clearInterval(id)
    }
  }, [])
  return rates
}

/**
 * Taux annuel de la devise (bps) que le pool a utilisé pour ce devis, en inversant
 * F = S × (1 + r_USD·t) / (1 + r_dev·t) après avoir retiré le spread.
 * Une seule équation pour deux taux : r_USD est supposé égal à `usdBps` (USTRY). Si le pool a un autre r_USD,
 * l'écart se reporte sur r_dev et le contrôle « out of sync » le détecte quand même.
 */
export function impliedRateBps(quote: Quote, side: SideTag, durationSecs: number, usdBps: number): number | null {
  if (durationSecs <= 0 || quote.forward_rate <= 0n || quote.spot <= 0n) return null
  const spread = side === 'BuyCurrency' ? 1 + POOL_SPREAD_BPS / 10_000 : 1 - POOL_SPREAD_BPS / 10_000
  const f = Number(quote.forward_rate) / spread
  const t = durationSecs / SECS_PER_YEAR
  const rDev = ((1 + (usdBps / 10_000) * t) * (Number(quote.spot) / f) - 1) / t
  return rDev * 10_000
}

/** bps → « 5.55% ». */
export function fmtBps(bps: number) {
  return `${(bps / 100).toFixed(2)}%`
}
