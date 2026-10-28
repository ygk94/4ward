// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT — Hugo] (logique fournie par Pricing) Devis : spot de l'oracle du pool (mock en démo), prix figé, marge à déposer (appel quote).
// Sous le devis : taux Etherfuse lus en direct et taux réellement utilisé par le pool (retrouvé depuis le devis).
import { LIVE_SPOT } from '../shared/config'
import { fmtRate, fmtUsdc, type Quote, type SideTag } from '../shared/contract'
import { ETHERFUSE_SITE, RATE_TOKEN, fmtBps, impliedRateBps, useEtherfuseRates, type EtherfuseRates } from '../shared/etherfuse'

export function QuoteCard({
  quote,
  loading,
  error,
  currency,
  side = 'BuyCurrency',
  durationSecs,
}: {
  quote: Quote | null
  loading: boolean
  error: string | null
  currency: string
  side?: SideTag
  durationSecs?: number
}) {
  const rates = useEtherfuseRates()
  if (error) return <div className="quote quote-err">{error}</div>
  return (
    <div className={`quote ${loading ? 'loading' : ''}`}>
      <p className="quote-l">{side === 'SellCurrency' ? 'You’ll receive, within your protection' : 'You’ll pay, within your protection'}</p>
      <p className="quote-big">
        {quote ? fmtUsdc(quote.usdc_cost, 2) : '—'} <span>USDC</span>
      </p>
      <dl className="recap">
        <div>
          <dt>Locked rate</dt>
          <dd>1 {currency} = {quote ? fmtRate(quote.forward_rate) : '—'} USD</dd>
        </div>
        <div>
          <dt>Today’s rate ({LIVE_SPOT ? 'Reflector FX' : 'demo oracle'})</dt>
          <dd>{quote ? fmtRate(quote.spot) : '—'} USD</dd>
        </div>
        <div className="strong">
          <dt>Deposit now</dt>
          <dd>{quote ? fmtUsdc(quote.margin, 2) : '—'} USDC</dd>
        </div>
      </dl>
      {rates && quote && durationSecs ? (
        <RatesLine rates={rates} quote={quote} currency={currency} side={side} durationSecs={durationSecs} />
      ) : null}
    </div>
  )
}

/** Écart toléré entre le taux du pool et Etherfuse avant d'afficher « out of sync ». */
const SYNC_TOLERANCE_BPS = 5

function RatesLine({ rates, quote, currency, side, durationSecs }: { rates: EtherfuseRates; quote: Quote; currency: string; side: SideTag; durationSecs: number }) {
  const token = RATE_TOKEN[currency]
  const usd = rates.bps.USTRY
  const live = token ? rates.bps[token] : undefined
  // Devise sans stablebond (ARS) ou absente de la réponse : pas de comparaison possible.
  if (token && live === undefined) return null
  const pool = impliedRateBps(quote, side, durationSecs, usd)
  const inSync = live === undefined || pool === null || Math.abs(pool - live) <= SYNC_TOLERANCE_BPS
  const time = rates.updatedAt.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })
  return (
    <p className="quote-rates tiny muted">
      Rates:{' '}
      <a href={ETHERFUSE_SITE} target="_blank" rel="noreferrer">
        Etherfuse live
      </a>{' '}
      · {token ? `${token} ${fmtBps(live!)}` : `${currency} fixed rate`} · USTRY {fmtBps(usd)} · updated {time}
      {pool !== null && (
        <>
          {' '}
          · pool {currency} {fmtBps(pool)}
          {!inSync && <b className="down"> · pool rate out of sync</b>}
        </>
      )}
    </p>
  )
}
