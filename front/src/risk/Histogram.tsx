// [FRONT — Hugo] Histogramme du Risk lab, grammaire des graphiques de data.morpho.org dans notre DA :
// grille de points, barres qui montent au premier rendu, infobulle HTML au survol, tirage précédent en fantôme.
import { useEffect, useId, useRef, useState, type CSSProperties, type PointerEvent as RPointerEvent } from 'react'
import { pct } from './format'
import { BIN_MAX, BIN_MIN, BIN_W, SENIOR_RATE, type Summary } from './montecarlo'

const W = 800
const H = 360
const PAD = { l: 8, r: 8, t: 12, b: 30 }
const IW = W - PAD.l - PAD.r
const IH = H - PAD.t - PAD.b
const BASE = PAD.t + IH
const TICKS = [-0.5, -0.25, 0, 0.25, 0.5]
/** Décalage d'apparition par barre et durée de l'animation (cf. risk.css). */
const STAGGER_MS = 15
const INTRO_MS = 500

const xOf = (v: number) => PAD.l + ((Math.min(BIN_MAX, Math.max(BIN_MIN, v)) - BIN_MIN) / (BIN_MAX - BIN_MIN)) * IW

interface Tip {
  i: number
  left: number
  top: number
  flip: boolean
}

export function Histogram({ s, prev, showPrev }: { s: Summary; prev: Summary | null; showPrev: boolean }) {
  const dots = `dots-${useId().replace(/[^a-zA-Z0-9_-]/g, '')}`
  const wrap = useRef<HTMLDivElement>(null)
  const svg = useRef<SVGSVGElement>(null)
  const [tip, setTip] = useState<Tip | null>(null)
  // Animation d'entrée au premier rendu seulement ; ensuite, seule la transition y/height joue.
  const [intro, setIntro] = useState(true)
  useEffect(() => {
    const t = setTimeout(() => setIntro(false), s.bins.length * STAGGER_MS + INTRO_MS + 50)
    return () => clearTimeout(t)
  }, [s.bins.length])

  const ghost = showPrev && prev ? prev : null
  // Échelle commune : le fantôme et le tirage courant se comparent à la même hauteur.
  const max = Math.max(...s.bins, ...(ghost?.bins ?? []))
  const total = s.bins.reduce((a, b) => a + b, 0)
  const bw = IW / s.bins.length
  const h = (c: number) => (max ? (c / max) * IH : 0)

  /** Place la carte au-dessus de la barre, en pixels du conteneur (le SVG garde son viewBox). */
  const show = (i: number) => {
    const el = svg.current
    const box = wrap.current?.getBoundingClientRect()
    const ctm = el?.getScreenCTM()
    if (!el || !box || !ctm) return
    const p = new DOMPoint(PAD.l + (i + 0.5) * bw, PAD.t).matrixTransform(ctm)
    const left = p.x - box.left
    setTip({ i, left, top: Math.max(0, p.y - box.top), flip: left > box.width - 220 })
  }
  const onMove = (i: number) => (e: RPointerEvent) => {
    if (e.pointerType === 'mouse') show(i)
  }
  // Tactile : pas de survol, l'infobulle s'ouvre au tap (et se referme au tap suivant sur la même barre).
  const onDown = (i: number) => (e: RPointerEvent) => {
    if (e.pointerType !== 'mouse') {
      if (tip?.i === i) setTip(null)
      else show(i)
    }
  }

  return (
    <div className="histo-wrap" ref={wrap}>
      <svg
        ref={svg}
        className={`histo ${intro ? 'intro' : ''} ${tip ? 'hovering' : ''}`}
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label="Distribution of the Dynamic tranche annual return"
        onPointerLeave={(e) => e.pointerType === 'mouse' && setTip(null)}
      >
        <defs>
          <pattern id={dots} width="16" height="16" patternUnits="userSpaceOnUse">
            <circle className="dot" cx="1" cy="1" r="1" />
          </pattern>
        </defs>
        <rect x={PAD.l} y={PAD.t} width={IW} height={IH} fill={`url(#${dots})`} />

        {ghost?.bins.map((c, i) =>
          c ? <rect key={`g${i}`} className="ghost" x={PAD.l + i * bw + 1} y={BASE - h(c)} width={bw - 2} height={h(c)} rx={2} /> : null,
        )}

        {tip && <line className="cursor" x1={PAD.l + (tip.i + 0.5) * bw} x2={PAD.l + (tip.i + 0.5) * bw} y1={PAD.t} y2={BASE} />}

        {s.bins.map((c, i) => (
          <rect
            key={i}
            className={`bar ${BIN_MIN + i * BIN_W < 0 ? 'loss' : 'gain'} ${tip?.i === i ? 'on' : ''}`}
            style={{ '--i': i } as CSSProperties}
            x={PAD.l + i * bw + 1}
            y={BASE - h(c)}
            width={bw - 2}
            height={h(c)}
            rx={2}
          />
        ))}

        <line className="axis" x1={PAD.l} x2={W - PAD.r} y1={BASE} y2={BASE} />
        {TICKS.map((t) => (
          <text key={t} className="tick" x={xOf(t)} y={H - 8} textAnchor={t === -0.5 ? 'start' : t === 0.5 ? 'end' : 'middle'}>
            {t === -0.5 ? '≤ −50%' : t === 0.5 ? '≥ +50%' : pct(t, 0)}
          </text>
        ))}
        <line className="marker safe" x1={xOf(SENIOR_RATE)} x2={xOf(SENIOR_RATE)} y1={PAD.t} y2={BASE} />
        <line className="marker mean" x1={xOf(s.mean)} x2={xOf(s.mean)} y1={PAD.t} y2={BASE} />

        {/* zones de survol pleine hauteur, au-dessus de tout */}
        {s.bins.map((_, i) => (
          <rect key={`h${i}`} className="hit" x={PAD.l + i * bw} y={PAD.t} width={bw} height={IH} onPointerMove={onMove(i)} onPointerDown={onDown(i)} />
        ))}
      </svg>

      {tip && <TipCard s={s} i={tip.i} total={total} style={{ left: tip.left, top: tip.top }} flip={tip.flip} />}
    </div>
  )
}

function TipCard({ s, i, total, style, flip }: { s: Summary; i: number; total: number; style: CSSProperties; flip: boolean }) {
  const lo = BIN_MIN + i * BIN_W
  const c = s.bins[i]
  const better = s.bins.slice(i + 1).reduce((a, b) => a + b, 0)
  // Une poignée d'années ne s'affiche pas « 0.0% » (ni toutes sauf quelques-unes « 100% »).
  const share = (n: number, digits: number) => {
    const x = (n / total) * 100
    const step = 10 ** -digits
    if (n > 0 && x < step) return `<${step}%`
    if (n < total && x > 100 - step) return `>${100 - step}%`
    return `${x.toFixed(digits)}%`
  }
  const tone = lo < 0 ? 'loss' : 'gain'
  return (
    <div className={`histo-tip ${flip ? 'flip' : ''}`} style={style} role="status">
      <p className="mono">
        {pct(lo)} → {pct(lo + BIN_W)}
      </p>
      <p>
        <i className={tone} />
        {share(c, 1)} of years
      </p>
      <p>
        <i className="muted-dot" />
        {share(better, 0)} of years do better
      </p>
      {i === 0 && s.under > 0 && (
        <p>
          <i className="loss" />
          includes {s.under.toLocaleString('en-US')} years below −50%
        </p>
      )}
      {i === s.bins.length - 1 && s.over > 0 && (
        <p>
          <i className="gain" />
          includes {s.over.toLocaleString('en-US')} years above +50%
        </p>
      )}
    </div>
  )
}
