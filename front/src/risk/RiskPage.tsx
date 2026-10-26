// [FRONT — Hugo] Page « Risk lab » : Monte-Carlo du pool de démarrage (modèle du README, sim/tranches.py).
// Vue simple : Safe contre Dynamic, l'histogramme et trois climats de marché. « Advanced » ouvre les curseurs,
// les indicateurs détaillés et les hypothèses. Recalcul dans un Web Worker.
import { useEffect, useRef, useState } from 'react'
import { useSlidingPill } from '../shared/useSlidingPill'
import { pct } from './format'
import { Histogram } from './Histogram'
import { CAP, DYNAMIC, SAFE, SENIOR_RATE, type Params, type Summary } from './montecarlo'
import './risk.css'
import { useTween } from './useTween'

const PATHS = 40_000
const DEFAULTS = { vol: 0.13, importers: 0.5, contracts: 40 }
// Vol réalisée sur 5 ans (cours BCE / Banco Nación, sim/out/calibration.md) ; 13 % = hypothèse prudente du README.
/** Les trois climats de la vue simple ; « Normal » = cas par défaut (+7,1 % / 15 % / −5 %). */
const MOODS = [
  { label: 'Calm', hint: 'like EUR', vol: 0.076 },
  { label: 'Normal', hint: 'model', vol: 0.13 },
  { label: 'Stormy', hint: 'like ARS', vol: 0.357 },
]
const VOL_PRESETS = [
  { label: 'EUR', vol: 0.076 },
  { label: 'MXN measured', vol: 0.11 },
  { label: 'Model', vol: 0.13 },
  { label: 'ARS', vol: 0.357 },
]

function useMonteCarlo(params: Params) {
  const worker = useRef<Worker | null>(null)
  const last = useRef(0)
  const current = useRef<Summary | null>(null)
  const [summary, setSummary] = useState<Summary | null>(null)
  // Tirage affiché juste avant : dessiné en fantôme pour comparer, jusqu'au calcul suivant.
  const [prev, setPrev] = useState<Summary | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    const w = new Worker(new URL('./mc.worker.ts', import.meta.url), { type: 'module' })
    w.onmessage = (e: MessageEvent<{ id: number; summary: Summary }>) => {
      // Seul le dernier calcul demandé compte : un curseur glissé vite en envoie plusieurs.
      if (e.data.id !== last.current) return
      setPrev(current.current)
      current.current = e.data.summary
      setSummary(e.data.summary)
      setBusy(false)
    }
    worker.current = w
    return () => w.terminate()
  }, [])

  const { vol, importers, contracts, seed } = params
  useEffect(() => {
    const t = setTimeout(() => {
      last.current += 1
      setBusy(true)
      worker.current?.postMessage({ id: last.current, params: { vol, importers, contracts, seed, paths: PATHS } })
    }, 60)
    return () => clearTimeout(t)
  }, [vol, importers, contracts, seed])

  return { summary, prev, busy }
}

export function RiskPage() {
  const [vol, setVol] = useState(DEFAULTS.vol)
  const [importers, setImporters] = useState(DEFAULTS.importers)
  const [contracts, setContracts] = useState(DEFAULTS.contracts)
  const [seed, setSeed] = useState(3)
  const [advanced, setAdvanced] = useState(false)
  const { summary: s, prev, busy } = useMonteCarlo({ vol, importers, contracts, seed })
  const [showPrev, setShowPrev] = useState(true)
  const { wrap: moodsWrap, pill: moodsPill } = useSlidingPill(vol)
  const { wrap: presetsWrap, pill: presetsPill } = useSlidingPill(`${vol}-${advanced}`)

  const mean = useTween(s?.mean)

  const reset = () => {
    setVol(DEFAULTS.vol)
    setImporters(DEFAULTS.importers)
    setContracts(DEFAULTS.contracts)
    setSeed(3)
  }
  const oneIn = s && s.loss > 0 ? Math.max(2, Math.round(1 / s.loss)) : null

  return (
    <div className={`risk-page ${advanced ? 'with-advanced' : ''}`}>
      <section className="risk-main">
        <header className="risk-head">
          <div>
            <p className="eyebrow">Risk lab {busy && <span className="chip">running…</span>}</p>
            <h1 className="risk-title">Safe or Dynamic?</h1>
          </div>
          <button className={`btn btn-ghost risk-adv-toggle ${advanced ? 'on' : ''}`} onClick={() => setAdvanced((v) => !v)}>
            {advanced ? 'Close advanced' : 'Advanced'}
          </button>
        </header>

        <div className="tranches">
          <article className="tranche-card safe">
            <p className="eyebrow">Safe</p>
            <p className="tranche-big">{pct(SENIOR_RATE)}</p>
            <p className="small">a year, fixed. Paid first.</p>
          </article>
          <article className="tranche-card dynamic">
            <p className="eyebrow">Dynamic</p>
            <p className="tranche-big">{mean !== undefined ? pct(mean) : '—'}</p>
            <p className="small">
              a year on average.{' '}
              {s && (oneIn ? `Loses money about 1 year in ${oneIn}.` : 'Never lost money in the simulation.')}
            </p>
          </article>
          <div className="moods has-pill" role="group" aria-label="Market conditions" ref={moodsWrap}>
            <span className="slide-pill" ref={moodsPill} aria-hidden />
            <p className="eyebrow">Market</p>
            {MOODS.map((m) => (
              <button key={m.label} className={Math.abs(vol - m.vol) < 1e-9 ? 'on' : ''} onClick={() => setVol(m.vol)}>
                {m.label} <span>{m.hint}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="risk-chart">
          <div className="histo-head">
            <p className="eyebrow">Dynamic return in 40,000 simulated years</p>
            <p className="histo-legend">
              <span className="safe">┆ Safe</span>
              <span className="mean">┆ Dynamic avg</span>
              {prev && showPrev && <span className="prev">▭ previous</span>}
              {prev && (
                <button className="chip" onClick={() => setShowPrev((v) => !v)}>
                  {showPrev ? 'Hide previous' : 'Show previous'}
                </button>
              )}
            </p>
          </div>
          {s ? <Histogram s={s} prev={prev} showPrev={showPrev} /> : <p className="muted small">Simulating…</p>}
          <p className="risk-foot tiny muted">Simulation of the target pool. Some features are on the roadmap: see Advanced.</p>
        </div>
      </section>

      {advanced && (
        <aside className="risk-side">
          <div className="kpis risk-kpis">
            <Kpi label="Losing years" value={s?.loss} fmt={(x) => pct(x, 0).replace('+', '')} hint="Dynamic below zero" />
            <Kpi label="Worst year in 20" value={s?.p5} fmt={(x) => pct(x, 0)} hint="5th percentile" />
            <Kpi label="Median year" value={s?.median} fmt={(x) => pct(x)} hint="Dynamic" />
            <Kpi label="Volume netted" value={s?.netted} fmt={(x) => pct(x, 0).replace('+', '')} hint={s ? `pool ${pct(s.pool)}` : '…'} />
          </div>

          <label className="field">
            <span>
              Currency volatility <b>{(vol * 100).toFixed(1)}%</b> / year
            </span>
            <input className="range" type="range" min={5} max={40} step={0.5} value={vol * 100} onChange={(e) => setVol(Number(e.target.value) / 100)} />
          </label>
          <div className="chips has-pill" ref={presetsWrap}>
            <span className="slide-pill" ref={presetsPill} aria-hidden />
            {VOL_PRESETS.map((p) => (
              <button key={p.label} className={Math.abs(vol - p.vol) < 1e-9 ? 'on' : ''} onClick={() => setVol(p.vol)}>
                {p.label} {(p.vol * 100).toFixed(1)}%
              </button>
            ))}
          </div>

          <label className="field">
            <span>
              Importers <b>{Math.round(importers * 100)}%</b> · exporters {Math.round((1 - importers) * 100)}%
            </span>
            <input className="range" type="range" min={50} max={100} step={5} value={importers * 100} onChange={(e) => setImporters(Number(e.target.value) / 100)} />
          </label>

          <label className="field">
            <span>
              Hedges per quarter <b>{contracts}</b> · {fmtK(contracts * 10_000)}
            </span>
            <input className="range" type="range" min={10} max={120} step={10} value={contracts} onChange={(e) => setContracts(Number(e.target.value))} />
          </label>

          <div className="risk-actions">
            <button className="btn btn-ghost" onClick={() => setSeed((x) => x + 1)}>
              New draw
            </button>
            <button className="btn btn-ghost" onClick={reset}>
              Reset
            </button>
          </div>

          <details className="risk-notes small muted">
            <summary>Assumptions</summary>
            <ul>
              <li>
                Pool: {fmtK(SAFE)} Safe + {fmtK(DYNAMIC)} Dynamic, $10k hedges at 90 days, cap ±{CAP * 100}%.
              </li>
              <li>Currency moves: normal at the chosen volatility, plus ±15% jumps (0.75% chance per quarter).</li>
              <li>Spread 0.4% per hedge, 80% to the pool. Idle capital in tokenized T-bills at 4%.</li>
              <li>
                <b>Roadmap</b>, not in the testnet MVP: netting importers against exporters, CETES hedging, T-bill yield.
              </li>
              <li>
                Volatility: 5-year realized, ECB and Banco Nación. Model: <span className="mono">sim/tranches.py</span>.
              </li>
            </ul>
          </details>
        </aside>
      )}
    </div>
  )
}

function fmtK(usd: number) {
  return `$${(usd / 1000).toLocaleString('en-US')}k`
}

function Kpi({ label, value, fmt, hint }: { label: string; value: number | undefined; fmt: (x: number) => string; hint: string }) {
  const shown = useTween(value)
  return (
    <div className="kpi">
      <p className="kpi-l">{label}</p>
      <p className="kpi-v">{shown !== undefined ? fmt(shown) : '—'}</p>
      <p className="kpi-h">{hint}</p>
    </div>
  )
}
