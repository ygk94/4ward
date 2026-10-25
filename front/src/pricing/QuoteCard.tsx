// [FRONT — Hugo] (logique fournie par Pricing) Devis : spot réel Reflector, prix figé, marge à déposer (appel quote).
import { fmtRate, fmtUsdc, type Quote } from '../shared/contract'

export function QuoteCard({ quote, loading, error, currency }: { quote: Quote | null; loading: boolean; error: string | null; currency: string }) {
  if (error) return <div className="quote quote-err">{error}</div>
  return (
    <div className={`quote ${loading ? 'loading' : ''}`}>
      <p className="quote-l">You’ll pay, no matter what</p>
      <p className="quote-big">
        {quote ? fmtUsdc(quote.usdc_cost, 2) : '—'} <span>USDC</span>
      </p>
      <dl className="recap">
        <div>
          <dt>Locked rate</dt>
          <dd>1 {currency} = {quote ? fmtRate(quote.forward_rate) : '—'} USD</dd>
        </div>
        <div>
          <dt>Spot today (Reflector)</dt>
          <dd>{quote ? fmtRate(quote.spot) : '—'} USD</dd>
        </div>
        <div className="strong">
          <dt>Margin to deposit now</dt>
          <dd>{quote ? fmtUsdc(quote.margin, 2) : '—'} USDC</dd>
        </div>
      </dl>
    </div>
  )
}
