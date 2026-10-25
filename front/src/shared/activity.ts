// [FRONT — Hugo] Statistiques d'usage de la page vault.
// - Couvertures : parcourues avec get_hedge (pas de limite de rétention) → PME, volume, encours, protection et durée moyennes.
// - Fournisseurs : déposants uniques des événements `lp_deposited` (le RPC garde ~7 jours), filtrés sur les parts actuelles.
import { rpc, scValToNative } from '@stellar/stellar-sdk'
import { config } from './config'
import { PRICE_DECIMALS, pool, read, type Hedge } from './contract'

export interface Activity {
  providers: number
  smes: number
  hedges: number
  openHedges: number
  /** Σ notionnels en USDC au taux bloqué (7 décimales). */
  volume: bigint
  openVolume: bigint
  avgCapBps: number
  avgTenorDays: number
  currencies: string[]
  recent: { id: bigint; hedge: Hedge }[]
}

/** Notionnel (devise, 7 déc.) × taux bloqué (USD, 14 déc.) → USDC, 7 déc. */
export function hedgeUsdc(h: Hedge): bigint {
  return (h.notional * h.locked_rate) / 10n ** BigInt(PRICE_DECIMALS)
}

export async function readAllHedges(max = 200): Promise<{ id: bigint; hedge: Hedge }[]> {
  const p = await pool()
  const out: { id: bigint; hedge: Hedge }[] = []
  for (let start = 1n; start <= BigInt(max); start += 8n) {
    const ids = Array.from({ length: 8 }, (_, i) => start + BigInt(i))
    const batch = await Promise.all(ids.map((id) => read(p.get_hedge({ hedge_id: id })).catch(() => undefined)))
    let done = false
    batch.forEach((h, i) => {
      if (!h) done = true
      else out.push({ id: ids[i], hedge: h })
    })
    if (done) break
  }
  return out
}

const LOOKBACK_LEDGERS = 120_000

async function depositors(): Promise<Set<string>> {
  const server = new rpc.Server(config.rpcUrl)
  const health = await server.getHealth()
  const filters: rpc.Api.EventFilter[] = [{ type: 'contract', contractIds: [config.poolId] }]
  const found = new Set<string>()
  let res = await server.getEvents({
    startLedger: Math.max(health.oldestLedger + 1, health.latestLedger - LOOKBACK_LEDGERS),
    filters,
    limit: 200,
  })
  // Le RPC ne parcourt qu'une fenêtre de ledgers par appel : on suit le curseur jusqu'au dernier ledger.
  for (let calls = 0; calls < 30; calls++) {
    for (const e of res.events) {
      if (scValToNative(e.topic[0]) === 'lp_deposited') found.add(String(scValToNative(e.topic[1])))
    }
    const ledger = Number(BigInt(res.cursor.split('-')[0]) >> 32n)
    if (res.events.length < 200 && ledger >= res.latestLedger - 1) break
    res = await server.getEvents({ cursor: res.cursor, filters, limit: 200 })
  }
  return found
}

export async function readActivity(): Promise<Activity> {
  const p = await pool()
  const [hedges, lps] = await Promise.all([readAllHedges(), depositors().catch(() => new Set<string>())])
  const shares = await Promise.all([...lps].map((lp) => read(p.lp_shares({ lp })).catch(() => 0n)))

  const open = hedges.filter(({ hedge }) => !hedge.settled)
  const sum = (xs: { hedge: Hedge }[]) => xs.reduce((s, { hedge }) => s + hedgeUsdc(hedge), 0n)
  const n = hedges.length || 1
  // Durée restante moyenne des couvertures ouvertes (la date d'ouverture n'est pas stockée).
  const now = Date.now() / 1000

  return {
    providers: shares.filter((s) => s > 0n).length,
    smes: new Set(hedges.map(({ hedge }) => hedge.owner)).size,
    hedges: hedges.length,
    openHedges: open.length,
    volume: sum(hedges),
    openVolume: sum(open),
    avgCapBps: hedges.reduce((s, { hedge }) => s + hedge.cap_bps, 0) / n,
    avgTenorDays: open.length ? open.reduce((s, { hedge }) => s + Math.max(0, Number(hedge.maturity) - now), 0) / open.length / 86_400 : 0,
    currencies: [...new Set(hedges.map(({ hedge }) => hedge.currency))],
    recent: hedges.slice(-4).reverse(),
  }
}
