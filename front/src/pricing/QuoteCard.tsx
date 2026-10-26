// [FRONT — Hugo] (logique fournie par Pricing) Devis : spot de l'oracle du pool (mock en démo), prix figé, marge à déposer (appel quote).
import { fmtRate, fmtUsdc, type Quote, type SideTag } from '../shared/contract'

export function QuoteCard({
  quote,
  loading,
  error,
  currency,
  side = 'BuyCurrency',
}: {
  quote: Quote | null
  loading: boolean
  error: string | null
  currency: string
  side?: SideTag
}) {
  if (error) return <div className="quote quote-err">{error}</div>
  return (
    <div className={`quote ${loading ? 'loading' : ''}`}>
      <p className="quote-l">{side === 'SellCurrency' ? 'You’ll receive, no matter what' : 'You’ll pay, no matter what'}</p>
      <p className="quote-big">
        {quote ? fmtUsdc(quote.usdc_cost, 2) : '—'} <span>USDC</span>
      </p>
      <dl className="recap">
        <div>
          <dt>Locked rate</dt>
          <dd>1 {currency} = {quote ? fmtRate(quote.forward_rate) : '—'} USD</dd>
        </div>
        <div>
          <dt>Today’s rate (demo oracle)</dt>
          <dd>{quote ? fmtRate(quote.spot) : '—'} USD</dd>
        </div>
        <div className="strong">
          <dt>Deposit now</dt>
          <dd>{quote ? fmtUsdc(quote.margin, 2) : '—'} USDC</dd>
        </div>
      </dl>
    </div>
  )
}
