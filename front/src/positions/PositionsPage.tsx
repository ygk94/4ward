// [FRONT — Hugo] Mes positions : protections (en cours / à régler / fermées) et position LP.
import { useCallback, useEffect, useState } from 'react'
import { fmtRate, fmtUsdc, readHedges, type Hedge } from '../shared/contract'
import { go } from '../shared/router'
import { TxStatus } from '../shared/TxStatus'
import { useTx } from '../shared/useTx'
import { WalletButton } from '../shared/TopBar'
import { useWallet } from '../shared/wallet'
import { readMine, type Mine } from '../vault/poolApi'

type Filter = 'all' | 'open' | 'closed'
type Status = 'open' | 'due' | 'closed'

function statusOf(h: Hedge, now: number): Status {
  if (h.settled) return 'closed'
  return Number(h.maturity) <= now ? 'due' : 'open'
}

const STATUS_LABEL: Record<Status, string> = { open: 'Open', due: 'Due · settle now', closed: 'Closed' }

export function PositionsPage() {
  const { address } = useWallet()
  const [hedges, setHedges] = useState<{ id: bigint; hedge: Hedge }[] | null>(null)
  const [lp, setLp] = useState<Mine | null>(null)
  const [filter, setFilter] = useState<Filter>('all')
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000))
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    if (!address) return
    try {
      const [hs, mine] = await Promise.all([readHedges(address), readMine(address)])
      setHedges(hs.reverse())
      setLp(mine)
      setError(null)
    } catch (e) {
      console.warn('[positions]', e)
      setError('Can’t read the contract right now.')
      setHedges([])
    }
  }, [address])

  // Relecture périodique : une couverture réglée par le keeper (scripts/keeper.sh) passe en « Closed » sans recharger.
  useEffect(() => {
    load()
    const id = setInterval(load, 8000)
    return () => clearInterval(id)
  }, [load])
  useEffect(() => {
    const id = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000)
    return () => clearInterval(id)
  }, [])

  if (!address) {
    return (
      <div className="positions empty-state">
        <h1 className="display small-display">My positions</h1>
        <p className="lede">Connect your wallet to see your hedges and your liquidity provider position.</p>
        <WalletButton />
      </div>
    )
  }

  const rows = (hedges ?? []).filter(({ hedge }) => {
    const s = statusOf(hedge, now)
    return filter === 'all' || (filter === 'closed' ? s === 'closed' : s !== 'closed')
  })

  return (
    <div className="positions">
      <header className="positions-head">
        <h1 className="display small-display">My positions</h1>
        <div className="seg">
          {(['all', 'open', 'closed'] as const).map((f) => (
            <button key={f} className={filter === f ? 'on' : ''} onClick={() => setFilter(f)}>
              {f === 'all' ? 'All' : f === 'open' ? 'Open' : 'Closed'}
            </button>
          ))}
        </div>
      </header>

      {lp && (lp.dynamic.shares > 0n || (lp.safe?.balance ?? 0n) > 0n) && (
        <article className="pos-card lp-card">
          <span className="status status-open">Liquidity provider</span>
          <div className="pos-main">
            <p className="pos-big">{fmtUsdc(lp.dynamic.balance + (lp.safe?.balance ?? 0n), 2)} USDC</p>
            {lp.dynamic.shares > 0n && (
              <p className="muted small">
                Dynamic: {fmtUsdc(lp.dynamic.balance, 2)} USDC · {fmtUsdc(lp.dynamic.shares, 2)} shares · valued at exit price
              </p>
            )}
            {(lp.safe?.balance ?? 0n) > 0n && <p className="muted small">Safe: {fmtUsdc(lp.safe?.balance ?? 0n, 2)} USDC</p>}
          </div>
          <button className="btn btn-ghost" onClick={() => go('/vault')}>
            Manage →
          </button>
        </article>
      )}

      {error && <p className="tx tx-err">{error}</p>}
      {hedges === null && <p className="muted">Loading positions…</p>}
      {hedges !== null && rows.length === 0 && (
        <div className="pos-empty">
          <p className="muted">No {filter === 'closed' ? 'closed ' : filter === 'open' ? 'open ' : ''}hedges yet.</p>
          <button className="btn btn-primary" onClick={() => go('/vault')}>
            Hedge an invoice →
          </button>
        </div>
      )}

      <div className="pos-list">
        {rows.map(({ id, hedge }) => (
          <HedgeRow key={String(id)} id={id} hedge={hedge} status={statusOf(hedge, now)} now={now} onSettled={load} />
        ))}
      </div>
    </div>
  )
}

function HedgeRow({ id, hedge, status, now, onSettled }: { id: bigint; hedge: Hedge; status: Status; now: number; onSettled: () => void }) {
  const { state, run } = useTx()
  const left = Number(hedge.maturity) - now
  const pay = hedge.side.tag === 'BuyCurrency'

  return (
    <article className={`pos-card st-${status}`}>
      <span className={`status status-${status}`}>{STATUS_LABEL[status]}</span>
      <div className="pos-main">
        <p className="pos-big">
          {fmtUsdc(hedge.notional)} {hedge.currency}
        </p>
        <p className="muted small">
          #{String(id)} · {pay ? 'to pay' : 'to receive'} · locked {fmtRate(hedge.locked_rate)} USD · protection ±{hedge.cap_bps / 100}%
        </p>
      </div>
      <div className="pos-side">
        <p className="small muted">Margin</p>
        <p>{fmtUsdc(hedge.margin, 2)} USDC</p>
        <p className="small muted">{status === 'open' ? `due in ${countdown(left)}` : new Date(Number(hedge.maturity) * 1000).toLocaleString('en-US')}</p>
      </div>
      {status === 'due' && (
        <div className="pos-action">
          <button
            className="btn btn-primary"
            disabled={state.step === 'signing'}
            onClick={async () => {
              await run((p) => p.settle({ hedge_id: id }))
              // Relire même en cas d'échec : si le keeper l'a déjà réglée (201), la ligne passe en « Closed ».
              onSettled()
            }}
          >
            {state.step === 'signing' ? 'Waiting…' : 'Settle'}
          </button>
          <TxStatus state={state} success={(r) => `Settled: ${fmtUsdc(r as bigint, 2)} USDC paid out.`} />
        </div>
      )}
    </article>
  )
}

function countdown(s: number) {
  if (s <= 0) return '0s'
  const d = Math.floor(s / 86_400)
  const h = Math.floor((s % 86_400) / 3600)
  const m = Math.floor((s % 3600) / 60)
  if (d > 0) return `${d}d ${h}h`
  if (h > 0) return `${h}h ${m}m`
  return `${m}m ${s % 60}s`
}
