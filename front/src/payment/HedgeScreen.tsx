// [FRONT — Hugo] (logique fournie par Paiement) Écran Lucía : facture, blocage du taux (open_hedge).
// Le règlement (settle) puis le paiement du fournisseur (path payment, payment/supplierPayment.ts) se font depuis « Mes positions ».
import { useEffect, useState } from 'react'
import { QuoteCard } from '../pricing/QuoteCard'
import { config, TRADABLE } from '../shared/config'
import { explainError, pool, read, toUnits, type Quote, type SideTag } from '../shared/contract'
import { go } from '../shared/router'
import { TxStatus } from '../shared/TxStatus'
import { useTx } from '../shared/useTx'

/** Protection minimale par devise, en % (miroir de set_min_cap du setup testnet). */
const MIN_CAP: Record<string, number> = { MXN: 5, BRL: 7, ARS: 15, EUR: 3 }

const CURRENCIES = TRADABLE
const DURATIONS = [
  { label: '30 days', secs: 30 * 86_400 },
  { label: '90 days', secs: 90 * 86_400 },
]

export function HedgeScreen({ onDone, onPick }: { onDone: () => void; onPick?: (currency: string, side: SideTag) => void }) {
  const [amount, setAmount] = useState('1000000')
  const [currency, setCurrency] = useState('MXN')
  const [side, setSide] = useState<SideTag>('BuyCurrency')
  const [duration, setDuration] = useState(DURATIONS[1].secs)
  const [cap, setCap] = useState(10)
  // Protection minimale par devise (set_min_cap dans scripts/setup-testnet.sh) : en dessous, le contrat refuse (#307).
  const minCap = MIN_CAP[currency] ?? 5
  useEffect(() => setCap((c) => Math.max(c, minCap)), [minCap])
  const [supplier, setSupplier] = useState(config.supplier ?? '')
  const [quote, setQuote] = useState<Quote | null>(null)
  const [quoteErr, setQuoteErr] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const { state, run } = useTx()

  // le globe du panneau pool relie le dollar à la devise choisie (cf. VaultGlobe)
  useEffect(() => {
    onPick?.(currency, side)
  }, [currency, side, onPick])

  // Saisie tolérante (espaces, « _ ») mais validée : toUnits/BigInt lèveraient pendant le rendu sur « 1,000,000 » ou « 1e6 ».
  const clean = amount.replace(/[\s_]/g, '')
  const valid = /^\d+([.,]\d{0,7})?$/.test(clean)
  const notional = valid ? Number(clean.replace(',', '.')) : 0
  const args = { currency, side: { tag: side }, notional: valid ? toUnits(clean) : 0n, duration_secs: BigInt(duration), cap_bps: cap * 100 }

  // Devis simulé (aucune signature), recalculé 400 ms après la dernière saisie.
  useEffect(() => {
    if (notional <= 0) {
      setQuote(null)
      setQuoteErr(null)
      setLoading(false)
      return
    }
    setLoading(true)
    const id = setTimeout(async () => {
      try {
        const p = await pool()
        setQuote(await read(p.quote(args)))
        setQuoteErr(null)
      } catch (e) {
        setQuote(null)
        setQuoteErr(explainError(e))
      } finally {
        setLoading(false)
      }
    }, 400)
    return () => clearTimeout(id)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [amount, currency, side, duration, cap])

  const submit = async () => {
    const id = await run((p, owner) => p.open_hedge({ owner, supplier, ...args }))
    if (id !== undefined) onDone()
  }

  return (
    <div className="form">
      <div className="seg">
        <button className={side === 'BuyCurrency' ? 'on' : ''} onClick={() => setSide('BuyCurrency')}>
          I have to pay
        </button>
        <button className={side === 'SellCurrency' ? 'on' : ''} onClick={() => setSide('SellCurrency')}>
          I will receive
        </button>
      </div>

      <label className="field">
        <span>Invoice amount</span>
        <div className="input-wrap">
          <input inputMode="decimal" value={amount} onChange={(e) => setAmount(e.target.value)} />
          <select value={currency} onChange={(e) => setCurrency(e.target.value)}>
            {CURRENCIES.map((c) => (
              <option key={c}>{c}</option>
            ))}
          </select>
        </div>
      </label>

      <label className="field">
        <span>Due in</span>
        <div className="chips">
          {DURATIONS.map((d) => (
            <button key={d.secs} className={duration === d.secs ? 'on' : ''} onClick={() => setDuration(d.secs)} type="button">
              {d.label}
            </button>
          ))}
        </div>
      </label>

      <label className="field">
        <span>
          Protection: <b>±{cap}%</b>
        </span>
        <input className="range" type="range" min={minCap} max={20} step={1} value={cap} onChange={(e) => setCap(Number(e.target.value))} />
        <small className="muted">
          Covered if the currency moves up to {cap}%. It’s also your max loss: the margin you deposit.
        </small>
      </label>

      {side === 'BuyCurrency' && (
        <label className="field">
          <span>Supplier address</span>
          <div className="input-wrap">
            <input className="mono" value={supplier} onChange={(e) => setSupplier(e.target.value.trim())} placeholder="G…" />
          </div>
        </label>
      )}

      <QuoteCard quote={quote} loading={loading} error={quoteErr} currency={currency} side={side} />

      <button className="btn btn-primary wide" disabled={!quote || notional <= 0 || state.step === 'signing' || !supplier} onClick={submit}>
        {state.step === 'signing' ? 'Waiting for wallet…' : 'Lock my price'}
      </button>
      <TxStatus state={state} success={(id) => `Hedge #${String(id)} opened.`} />
      {state.step === 'done' && (
        <button className="link" onClick={() => go('/positions')}>
          See my positions →
        </button>
      )}
    </div>
  )
}
