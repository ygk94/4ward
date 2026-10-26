// [FRONT — Hugo] Panneau pool (2/3 gauche de la page vault), tout tient dans l'écran :
// capital, usage (fournisseurs, PME, volume, encours), utilisation, équilibre du pool par devise, dernières couvertures.
// En haut à droite, le petit vault devient un globe quand la PME couvre une facture (cf. VaultGlobe).
import { useEffect, useState } from 'react'
import { hedgeUsdc, type Activity } from '../shared/activity'
import { fmtUsdc, fromUnits, pool, read, type Exposure, type PoolStats } from '../shared/contract'
import { shortAddress, useWallet } from '../shared/wallet'
import { VaultGlobe } from '../visual/VaultGlobe'
import { readSplit } from './poolApi'

interface Props {
  stats: PoolStats | null
  demo: boolean
  activity: Activity | null
  globe?: boolean
  link?: [string, string]
}

export function PoolPanel({ stats, demo, activity, globe = false, link }: Props) {
  const { address } = useWallet()
  const [mine, setMine] = useState<{ balance: bigint; shares: bigint } | null>(null)
  // Capital réellement bloqué par les couvertures (pris sur la tranche Dynamic), relu à chaque rafraîchissement.
  const [locked, setLocked] = useState<bigint | null>(null)

  useEffect(() => {
    if (!stats || demo) {
      setLocked(null)
      return
    }
    readSplit()
      .then((s) => setLocked(s.locked))
      .catch(() => setLocked(null))
  }, [stats, demo])

  useEffect(() => {
    if (!address) {
      setMine(null)
      return
    }
    pool()
      .then(async (p) => ({
        balance: await read(p.lp_balance({ lp: address })),
        shares: await read(p.lp_shares({ lp: address })),
      }))
      .then(setMine)
      .catch(() => setMine(null))
  }, [address, stats])

  // capacity = junior − réservé (vault.rs) : totalAssets − capacity compterait aussi tout le capital Safe.
  // Hors ligne (données de démo, sans tranche Safe), junior = totalAssets.
  const reserved = locked ?? (stats ? stats.totalAssets - stats.capacity : 0n)
  const usage = stats && stats.totalAssets > 0n ? Number((reserved * 10_000n) / stats.totalAssets) / 100 : 0
  const raw = stats ? (fromUnits(stats.sharePrice) - 1) * 100 : 0
  // Arrondi d'affichage : un écart sous 0,005 % s'affiche +0%, pas -0%.
  const lpReturn = Math.abs(raw) < 0.005 ? 0 : raw
  const a = activity
  const pct = (n: number, digits = 1) => n.toLocaleString('en-US', { maximumFractionDigits: digits })

  return (
    <section className="pool-panel">
      <header className="pool-head">
        <div className="pool-title">
          <p className="eyebrow">
            USDC vault · FX hedges {demo && <span className="chip">demo</span>}
          </p>
          <h1 className="pool-tvl">
            {stats ? fmtUsdc(stats.totalAssets) : '—'} <span>USDC</span>
          </h1>
          <p className="muted small">
            Total value locked · LP return since launch, measured on-chain{' '}
            <b className={lpReturn >= 0 ? 'up' : 'down'}>
              {lpReturn >= 0 ? '+' : ''}
              {pct(lpReturn, 2)}%
            </b>
            {mine && mine.shares > 0n && (
              <>
                {' '}
                · your position <b>{fmtUsdc(mine.balance, 2)} USDC</b>
              </>
            )}
          </p>
        </div>
        <VaultGlobe className={`pool-vault ${globe ? 'is-globe' : ''}`} globe={globe} from={link?.[0]} to={link?.[1]} />
      </header>

      <div className="kpis">
        <Kpi label="Liquidity providers" value={a ? String(a.providers) : '—'} hint="wallets with shares" />
        <Kpi label="SMEs hedged" value={a ? String(a.smes) : '—'} hint={a ? `${a.hedges} hedges opened` : '…'} />
        <Kpi label="Hedged volume" value={a ? fmtUsdc(a.volume) : '—'} unit="USDC" hint="all-time, at locked rate" />
        <Kpi label="Open coverage" value={a ? fmtUsdc(a.openVolume) : '—'} unit="USDC" hint={a ? `${a.openHedges} open hedges` : '…'} />
        <Kpi label="Utilization" value={pct(usage)} unit="%" hint={`${fmtUsdc(reserved)} USDC reserved`} bar={usage} />
        <Kpi label="Available capacity" value={stats ? fmtUsdc(stats.capacity) : '—'} unit="USDC" hint="for new hedges" />
        <Kpi
          label="Share price"
          value={stats ? fmtUsdc(stats.sharePrice, 4) : '—'}
          unit="USDC"
          hint={stats ? `exit ${fmtUsdc(stats.exitPrice, 4)}` : '…'}
        />
        <Kpi
          label="Avg. protection"
          value={a && a.hedges ? `±${pct(a.avgCapBps / 100)}` : '—'}
          unit="%"
          hint={a && a.openHedges ? `${pct(a.avgTenorDays)} days to maturity` : 'spread 0.50%'}
        />
      </div>

      <div className="pool-bottom">
        <PoolBalance stats={stats} />
        <div className="mini-card">
          <h3>Recent hedges</h3>
          {!a && <p className="muted small">Loading…</p>}
          {a && a.recent.length === 0 && <p className="muted small">No hedge opened yet. Be the first SME.</p>}
          {a?.recent.map(({ id, hedge }) => (
            <Row
              key={String(id)}
              label={`#${id} · ${shortAddress(hedge.owner)} · ±${hedge.cap_bps / 100}%`}
              value={`${fmtUsdc(hedgeUsdc(hedge))} USDC`}
              tag={hedge.settled ? 'settled' : 'open'}
            />
          ))}
        </div>
      </div>
    </section>
  )
}

/**
 * Équilibre du pool, devise par devise : PME qui paient dans la devise (USDC → devise, le pool perd si elle monte)
 * contre PME payées dans la devise (devise → USDC, le pool perd si elle baisse). Ce qui se compense ne coûte
 * rien au pool : seul le net est un vrai risque. « Offset » = part du volume ouvert qui s'annule.
 */
function PoolBalance({ stats }: { stats: PoolStats | null }) {
  const rows = stats?.exposures ?? []
  const gross = rows.reduce((t, { exp }) => t + exp.buy_notional + exp.sell_notional, 0n)
  const matched = rows.reduce((t, { exp }) => t + 2n * min(exp.buy_notional, exp.sell_notional), 0n)
  const offset = gross > 0n ? Number((matched * 1000n) / gross) / 10 : null

  return (
    <div className="mini-card balance">
      <div className="balance-head">
        <h3>Pool balance</h3>
        <p className="small muted">
          {offset === null ? 'no open hedges' : (
            <>
              <b>{offset.toLocaleString('en-US', { maximumFractionDigits: 1 })}%</b> offset
            </>
          )}
        </p>
      </div>
      {rows.map(({ currency, exp }) => (
        <BalanceRow key={currency} currency={currency} exp={exp} />
      ))}
      <p className="balance-key tiny muted">
        <i className="pay" /> SMEs pay in it · <i className="get" /> SMEs get paid in it
      </p>
    </div>
  )
}

function BalanceRow({ currency, exp }: { currency: string; exp: Exposure }) {
  const total = exp.buy_notional + exp.sell_notional
  const buy = total > 0n ? Number((exp.buy_notional * 1000n) / total) / 10 : 0
  const net = exp.net_notional
  const title =
    total === 0n
      ? `No open ${currency} hedge`
      : `USDC → ${currency}: ${fmtUsdc(exp.buy_notional)} USDC · ${currency} → USDC: ${fmtUsdc(exp.sell_notional)} USDC · net ${fmtUsdc(net < 0n ? -net : net)} USDC, pool loses if ${currency} ${net >= 0n ? 'rises' : 'falls'}`
  return (
    <div className="balance-row" title={title}>
      <span className="balance-cur">{currency}</span>
      <div className={`balance-bar ${total === 0n ? 'empty' : ''}`}>
        {total > 0n && (
          <>
            <div className="pay" style={{ width: `${buy}%` }} />
            <div className="get" style={{ width: `${100 - buy}%` }} />
          </>
        )}
      </div>
      <b>{total === 0n ? '—' : `${Math.round(buy)} / ${Math.round(100 - buy)}`}</b>
    </div>
  )
}

const min = (a: bigint, b: bigint) => (a < b ? a : b)

function Kpi({ label, value, unit, hint, bar }: { label: string; value: string; unit?: string; hint: string; bar?: number }) {
  return (
    <div className="kpi">
      <p className="kpi-l">{label}</p>
      <p className="kpi-v">
        {value} {unit && <span>{unit}</span>}
      </p>
      {bar !== undefined && (
        <div className="kpi-bar">
          <div style={{ width: `${Math.min(bar, 100)}%` }} />
        </div>
      )}
      <p className="kpi-h">{hint}</p>
    </div>
  )
}

function Row({ label, value, strong, tag }: { label: string; value: string; strong?: boolean; tag?: 'open' | 'settled' }) {
  return (
    <div className={`row ${strong ? 'strong' : ''}`}>
      <span>
        {tag && <i className={`tag tag-${tag}`}>{tag}</i>}
        {label}
      </span>
      <b>{value}</b>
    </div>
  )
}
