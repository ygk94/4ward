// [FRONT — Hugo] Mes positions : protections (en cours / à régler / fermées) et position LP.
import { useCallback, useEffect, useState } from 'react'
import { explainError, fmtRate, fmtUsdc, pool, read, readHedges, PRICE_DECIMALS, type Hedge } from '../shared/contract'
import {
  PAYABLE,
  paySupplier,
  quoteSupplierPayment,
  recallPaid,
  recallPayout,
  rememberPaid,
  rememberPayout,
  type PaidRecord,
} from '../payment/supplierPayment'
import { go } from '../shared/router'
import { TxStatus } from '../shared/TxStatus'
import { explorerTx, useTx } from '../shared/useTx'
import { WalletButton } from '../shared/TopBar'
import { useWallet } from '../shared/wallet'
import { readMine, type Mine } from '../vault/poolApi'
import { ProtectedBadge } from '../visual/Shield'

type Filter = 'all' | 'open' | 'closed'
type Status = 'open' | 'due' | 'closed'

// settle compare l'échéance à l'heure du ledger, en retard de quelques secondes sur le navigateur :
// on attend cette marge avant de proposer Settle (sinon refus #202).
const LEDGER_LAG_SECS = 10

function statusOf(h: Hedge, now: number): Status {
  if (h.settled) return 'closed'
  return Number(h.maturity) + LEDGER_LAG_SECS <= now ? 'due' : 'open'
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
  const { address } = useWallet()
  const left = Number(hedge.maturity) - now
  const pay = hedge.side.tag === 'BuyCurrency'
  // Clôture anticipée : on simule d'abord close_early pour montrer ce que la PME récupérerait, puis on confirme.
  const [preview, setPreview] = useState<{ payout: bigint } | { error: string } | null>(null)

  async function previewClose() {
    if (!address) return
    try {
      setPreview({ payout: await read((await pool(address)).close_early({ hedge_id: id })) })
    } catch (e) {
      setPreview({ error: explainError(e) })
    }
  }

  return (
    <article className={`pos-card st-${status}`}>
      <div className="pos-tags">
        <span className={`status status-${status}`}>{STATUS_LABEL[status]}</span>
        {status !== 'closed' && <ProtectedBadge capBps={hedge.cap_bps} />}
      </div>
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
              const payout = await run((p) => p.settle({ hedge_id: id }))
              if (typeof payout === 'bigint') rememberPayout(id, payout)
              // Relire même en cas d'échec : si le keeper l'a déjà réglée (201), la ligne passe en « Closed ».
              onSettled()
            }}
          >
            {state.step === 'signing' ? 'Waiting…' : 'Settle'}
          </button>
          <TxStatus state={state} success={(r) => `Settled: ${fmtUsdc(r as bigint, 2)} USDC paid out.`} />
        </div>
      )}
      {status === 'open' && (
        <div className="pos-action">
          {!preview && state.step !== 'done' && (
            <button className="btn btn-ghost" onClick={previewClose}>
              Close early
            </button>
          )}
          {preview && 'error' in preview && <p className="tx tx-err">{preview.error}</p>}
          {preview && 'payout' in preview && state.step !== 'done' && (
            <>
              <p className="small">
                Close now at today’s forward rate: you get back <b>{fmtUsdc(preview.payout, 2)} USDC</b> of your{' '}
                {fmtUsdc(hedge.margin, 2)} USDC margin
                {preview.payout >= hedge.margin
                  ? ' (the rate moved your way).'
                  : ' (the difference covers the market move and the exit spread, and goes to the pool).'}
              </p>
              <button
                className="btn btn-primary"
                disabled={state.step === 'signing'}
                onClick={async () => {
                  const payout = await run((p) => p.close_early({ hedge_id: id }))
                  if (typeof payout === 'bigint') rememberEarly(id)
                  setPreview(null)
                  onSettled()
                }}
              >
                {state.step === 'signing' ? 'Waiting…' : 'Confirm close'}
              </button>
              <button className="btn btn-ghost" disabled={state.step === 'signing'} onClick={() => setPreview(null)}>
                Keep it
              </button>
            </>
          )}
          <TxStatus state={state} success={(r) => `Closed early: ${fmtUsdc(r as bigint, 2)} USDC paid out.`} />
        </div>
      )}
      {status === 'closed' && pay && PAYABLE.includes(hedge.currency) && !recallEarly(id) && (
        <SupplierPayment id={id} hedge={hedge} />
      )}
    </article>
  )
}

// Une couverture clôturée par anticipation n'a pas de facture échue à payer : on la marque pour ne pas proposer le paiement.
function rememberEarly(id: bigint) {
  try {
    localStorage.setItem(`fx.early.${id}`, '1')
  } catch {
    // stockage indisponible
  }
}
function recallEarly(id: bigint) {
  try {
    return localStorage.getItem(`fx.early.${id}`) === '1'
  } catch {
    return false
  }
}

/**
 * Dernière étape du parcours : après le règlement, payer le fournisseur en devise (path payment strict receive).
 * Coût total = USDC payés sur le DEX − P&L de la couverture (versement du règlement − marge déposée).
 */
function SupplierPayment({ id, hedge }: { id: bigint; hedge: Hedge }) {
  const [paid, setPaid] = useState<PaidRecord | null>(() => recallPaid(id))
  const [estimate, setEstimate] = useState<bigint | null>(null)
  const [busy, setBusy] = useState<'quote' | 'signing' | null>(null)
  const [error, setError] = useState<string | null>(null)
  const notional = `${fmtUsdc(hedge.notional)} ${hedge.currency}`

  async function preview() {
    setBusy('quote')
    setError(null)
    try {
      setEstimate((await quoteSupplierPayment(hedge.currency, hedge.notional)).usdcCost)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(null)
    }
  }

  async function pay() {
    setBusy('signing')
    setError(null)
    try {
      const rec = await paySupplier(hedge.currency, hedge.notional, hedge.supplier)
      rememberPaid(id, rec)
      setPaid(rec)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(null)
    }
  }

  if (paid) {
    const payout = recallPayout(id)
    const locked = (hedge.notional * hedge.locked_rate) / 10n ** BigInt(PRICE_DECIMALS)
    const pnl = payout === null ? null : payout - hedge.margin
    return (
      <div className="pos-action">
        <p className="tx tx-ok">
          Your supplier received exactly {notional}. You paid {fmtUsdc(paid.usdcPaid, 2)} USDC on the Stellar DEX
          {pnl === null ? (
            '.'
          ) : (
            <>
              {' '}
              and your protection {pnl >= 0n ? 'paid you' : 'cost you'} {fmtUsdc(pnl >= 0n ? pnl : -pnl, 2)} USDC: total
              cost <b>{fmtUsdc(paid.usdcPaid - pnl, 2)} USDC</b> (locked price {fmtUsdc(locked, 2)} USDC).
            </>
          )}{' '}
          <a href={explorerTx(paid.hash)} target="_blank" rel="noreferrer">
            View on explorer ↗
          </a>
        </p>
      </div>
    )
  }

  return (
    <div className="pos-action">
      {estimate === null ? (
        <button className="btn btn-primary" disabled={busy !== null} onClick={preview}>
          {busy === 'quote' ? 'Checking the DEX…' : 'Pay my supplier'}
        </button>
      ) : (
        <>
          <p className="small">
            Your supplier receives exactly <b>{notional}</b>. Cost on the Stellar DEX right now: about{' '}
            {fmtUsdc(estimate, 2)} USDC (at most 0.5% more if the price moves while you sign).
          </p>
          <button className="btn btn-primary" disabled={busy !== null} onClick={pay}>
            {busy === 'signing' ? 'Waiting…' : `Pay ${notional}`}
          </button>
          <button className="btn btn-ghost" disabled={busy !== null} onClick={() => setEstimate(null)}>
            Not now
          </button>
        </>
      )}
      {busy === 'signing' && <p className="tx tx-wait">Confirm the payment in your wallet…</p>}
      {error && <p className="tx tx-err">{error}</p>}
    </div>
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
