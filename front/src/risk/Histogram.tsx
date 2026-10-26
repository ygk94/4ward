// [FRONT — Hugo] Histogramme du Risk lab, grammaire des graphiques de data.morpho.org dans notre DA :
// grille de points, infobulle HTML au survol, tirage précédent en fantôme. Au premier rendu, ce sont les arêtes du vault
// de fond qui dessinent les barres (cf. burst.ts) : on leur publie la position des barres, puis on les révèle.
// Focus sur un repère (Safe ou Dynamic avg) : au survol de sa ligne ou au clic dans la légende, les barres
// s'écartent de part et d'autre et la place libérée accueille une petite carte d'infos.
import { useEffect, useId, useRef, useState, type CSSProperties, type PointerEvent as RPointerEvent } from 'react'
import { risk, riskProgress } from '../visual/burst'
import { pct } from './format'
import { BIN_MAX, BIN_MIN, BIN_W, SENIOR_RATE, type Summary } from './montecarlo'

const W = 800
const H = 360
const PAD = { l: 8, r: 8, t: 12, b: 30 }
const IW = W - PAD.l - PAD.r
const IH = H - PAD.t - PAD.b
const BASE = PAD.t + IH
/** Les barres sont des groupes de points sur une grille régulière : COLS colonnes par barre (cf. PolyVault, éclats). */
const COLS = 4
const TICKS = [-0.5, -0.25, 0, 0.25, 0.5]
/** Avancement de la cinématique (0 → 1) à partir duquel les vraies barres apparaissent. */
const REVEAL_AT = 0.8
/** Largeur libérée autour du repère en focus (unités du viewBox), et distance de « prise » de la ligne. */
const GAP = 200
const GRAB = 14

export type Focus = 'safe' | 'mean'

const xOf = (v: number) => PAD.l + ((Math.min(BIN_MAX, Math.max(BIN_MIN, v)) - BIN_MIN) / (BIN_MAX - BIN_MIN)) * IW

/**
 * Écartement : tout l'axe est resserré de GAP, puis ce qui est à droite de `split` (une frontière de barre)
 * est poussé de GAP. Aucune barre n'est coupée ; sans focus, c'est l'identité.
 */
function spreadOf(split: number | null) {
  if (split === null) return { map: (x: number) => x, unmap: (x: number) => x, k: 1, gap: null }
  const k = (IW - GAP) / IW
  const g0 = PAD.l + (split - PAD.l) * k
  return {
    map: (x: number) => PAD.l + (x - PAD.l) * k + (x > split + 1e-6 ? GAP : 0),
    unmap: (x: number) => (x <= g0 ? PAD.l + (x - PAD.l) / k : x >= g0 + GAP ? PAD.l + (x - GAP - PAD.l) / k : split),
    k,
    gap: { x0: g0, x1: g0 + GAP },
  }
}

interface Tip {
  i: number
  left: number
  top: number
  flip: boolean
}

export function Histogram({
  s,
  prev,
  showPrev,
  pinned,
}: {
  s: Summary
  prev: Summary | null
  showPrev: boolean
  pinned: Focus | null
}) {
  const uid = useId().replace(/[^a-zA-Z0-9_-]/g, '')
  const dots = `dots-${uid}`
  const gainDots = `gain-${uid}`
  const lossDots = `loss-${uid}`
  const wrap = useRef<HTMLDivElement>(null)
  const svg = useRef<SVGSVGElement>(null)
  const [tip, setTip] = useState<Tip | null>(null)
  const [hover, setHover] = useState<Focus | null>(null)
  const [reveal, setReveal] = useState(false)
  const geom = useRef({ bins: s.bins, max: 0, bw: 0 })

  const ghost = showPrev && prev ? prev : null
  // Échelle commune : le fantôme et le tirage courant se comparent à la même hauteur.
  const max = Math.max(...s.bins, ...(ghost?.bins ?? []))
  const total = s.bins.reduce((a, b) => a + b, 0)
  const bw = IW / s.bins.length
  const h = (c: number) => (max ? (c / max) * IH : 0)
  // pas de la grille de points ; la hauteur d'une barre est arrondie à un nombre entier de rangées
  const cell = bw / COLS
  const hRows = (c: number) => Math.floor(h(c) / cell + 1e-6) * cell
  useEffect(() => {
    geom.current = { bins: s.bins, max, bw }
  })

  // Publie la position des barres tant que dure la cinématique (la page bouge encore à son entrée), puis les révèle.
  useEffect(() => {
    let raf = 0
    setReveal(false) // la cinématique repart de zéro (rechargement à chaud, ré-exécution de l'effet) : les vraies barres se recachent
    const tick = () => {
      const ctm = svg.current?.getScreenCTM()
      const { bins, max, bw } = geom.current
      if (ctm && max) {
        risk.rects = bins.map((c, i) => {
          const x0 = PAD.l + i * bw
          const tl = new DOMPoint(x0, BASE - (c / max) * IH).matrixTransform(ctm)
          const br = new DOMPoint(x0 + bw, BASE).matrixTransform(ctm)
          return {
            x0: tl.x / window.innerWidth,
            x1: br.x / window.innerWidth,
            top: tl.y / window.innerHeight,
            bottom: br.y / window.innerHeight,
            loss: BIN_MIN + i * BIN_W < 0,
          }
        })
        if (!risk.start) risk.start = performance.now()
      }
      const p = riskProgress()
      if (p >= REVEAL_AT) setReveal(true)
      if (p < 1) raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => {
      cancelAnimationFrame(raf)
      risk.rects = []
      risk.start = 0
    }
  }, [])

  const value = (f: Focus) => (f === 'safe' ? SENIOR_RATE : s.mean)
  const focus = hover ?? pinned
  // La barre qui contient le repère reste du côté où se trouve son centre.
  const split = focus ? PAD.l + Math.round((xOf(value(focus)) - PAD.l) / bw) * bw : null
  const sp = spreadOf(split)
  const bx = (i: number) => sp.map(PAD.l + i * bw)
  const markerX = (f: Focus) => (f === focus && sp.gap ? (sp.gap.x0 + sp.gap.x1) / 2 : sp.map(xOf(value(f))))

  /** Coordonnée x du pointeur dans le viewBox. */
  const toView = (e: RPointerEvent) => {
    const ctm = svg.current?.getScreenCTM()
    return ctm ? new DOMPoint(e.clientX, e.clientY).matrixTransform(ctm.inverse()).x : null
  }

  /** Place la carte au-dessus de la barre, en pixels du conteneur (le SVG garde son viewBox). */
  const showBar = (i: number) => {
    const el = svg.current
    const box = wrap.current?.getBoundingClientRect()
    const ctm = el?.getScreenCTM()
    if (!el || !box || !ctm) return
    const p = new DOMPoint(bx(i) + (bw * sp.k) / 2, PAD.t).matrixTransform(ctm)
    const left = p.x - box.left
    setTip({ i, left, top: Math.max(0, p.y - box.top), flip: left > box.width - 220 })
  }

  const onMove = (e: RPointerEvent) => {
    if (e.pointerType !== 'mouse') return
    const x = toView(e)
    if (x === null) return
    // Déjà en focus au survol : on le garde tant que le pointeur reste dans l'espace libéré.
    if (hover && sp.gap && x >= sp.gap.x0 - GRAB && x <= sp.gap.x1 + GRAB) return
    const near = (['safe', 'mean'] as Focus[]).find((f) => Math.abs(x - markerX(f)) < GRAB) ?? null
    if (near) {
      setHover(near)
      setTip(null)
      return
    }
    if (hover) setHover(null)
    if (sp.gap && x > sp.gap.x0 && x < sp.gap.x1) {
      setTip(null)
      return
    }
    const i = Math.floor((sp.unmap(x) - PAD.l) / bw)
    if (i >= 0 && i < s.bins.length) showBar(i)
    else setTip(null)
  }
  // Tactile : pas de survol, l'infobulle de barre s'ouvre au tap (le focus passe par la légende).
  const onDown = (e: RPointerEvent) => {
    if (e.pointerType === 'mouse') return
    const x = toView(e)
    if (x === null || (sp.gap && x > sp.gap.x0 && x < sp.gap.x1)) return
    const i = Math.floor((sp.unmap(x) - PAD.l) / bw)
    if (tip?.i === i || i < 0 || i >= s.bins.length) setTip(null)
    else showBar(i)
  }

  // Carte du focus, posée dans l'espace libéré (en pixels du conteneur).
  const [gapBox, setGapBox] = useState<{ left: number; width: number; top: number } | null>(null)
  const gx0 = sp.gap?.x0
  useEffect(() => {
    if (gx0 === undefined) return
    const raf = requestAnimationFrame(() => {
      const box = wrap.current?.getBoundingClientRect()
      const ctm = svg.current?.getScreenCTM()
      if (!box || !ctm) return
      const a = new DOMPoint(gx0, PAD.t).matrixTransform(ctm)
      const b = new DOMPoint(gx0 + GAP, PAD.t).matrixTransform(ctm)
      setGapBox({ left: a.x - box.left, width: b.x - a.x, top: a.y - box.top })
    })
    return () => cancelAnimationFrame(raf)
  }, [gx0])

  return (
    <div className="histo-wrap" ref={wrap}>
      <svg
        ref={svg}
        className={`histo ${reveal ? 'drawn' : ''} ${tip ? 'hovering' : ''} ${focus ? `focus-${focus}` : ''}`}
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label="Distribution of the Dynamic tranche annual return"
        onPointerMove={onMove}
        onPointerDown={onDown}
        onPointerLeave={(e) => {
          if (e.pointerType !== 'mouse') return
          setTip(null)
          setHover(null)
        }}
      >
        <defs>
          <pattern id={dots} width="16" height="16" patternUnits="userSpaceOnUse">
            <circle className="dot" cx="1" cy="1" r="1" />
          </pattern>
          {/* grille des barres : cellule bw / COLS alignée sur la ligne de base, comme les éclats de l'entrée */}
          {[gainDots, lossDots].map((id) => (
            <pattern key={id} id={id} x={PAD.l} y={BASE - cell} width={cell} height={cell} patternUnits="userSpaceOnUse">
              <circle className={id === gainDots ? 'pd gain' : 'pd loss'} cx={cell / 2} cy={cell / 2} r={1.8} />
            </pattern>
          ))}
        </defs>
        <rect x={PAD.l} y={PAD.t} width={IW} height={IH} fill={`url(#${dots})`} />

        {ghost?.bins.map((c, i) =>
          c ? <rect key={`g${i}`} className="ghost" x={bx(i) + 1} y={BASE - h(c)} width={bw * sp.k - 2} height={h(c)} rx={2} /> : null,
        )}

        {tip && <Vline className="cursor" x={bx(tip.i) + (bw * sp.k) / 2} />}

        {s.bins.map((c, i) => (
          <rect
            key={i}
            className={`bar ${BIN_MIN + i * BIN_W < 0 ? 'loss' : 'gain'} ${tip?.i === i ? 'on' : ''}`}
            style={{ fill: `url(#${BIN_MIN + i * BIN_W < 0 ? lossDots : gainDots})` }}
            x={bx(i)}
            y={BASE - hRows(c)}
            width={bw * sp.k}
            height={hRows(c)}
          />
        ))}

        <line className="axis" x1={PAD.l} x2={W - PAD.r} y1={BASE} y2={BASE} />
        {TICKS.map((t) => (
          <text
            key={t}
            className="tick"
            x={0}
            y={H - 8}
            style={{ transform: `translateX(${sp.map(xOf(t))}px)` }}
            textAnchor={t === -0.5 ? 'start' : t === 0.5 ? 'end' : 'middle'}
          >
            {t === -0.5 ? '≤ −50%' : t === 0.5 ? '≥ +50%' : pct(t, 0)}
          </text>
        ))}
        <Vline className={`marker safe ${focus === 'safe' ? 'on' : ''}`} x={markerX('safe')} />
        <Vline className={`marker mean ${focus === 'mean' ? 'on' : ''}`} x={markerX('mean')} />
      </svg>

      {focus && gapBox && (
        <FocusCard
          key={focus}
          focus={focus}
          s={s}
          total={total}
          style={{ left: gapBox.left, width: gapBox.width, top: gapBox.top }}
        />
      )}
      {tip && !focus && <TipCard s={s} i={tip.i} total={total} style={{ left: tip.left, top: tip.top }} flip={tip.flip} />}
    </div>
  )
}

/** Ligne verticale déplacée par transform : la transition CSS la fait glisser avec les barres. */
function Vline({ className, x }: { className: string; x: number }) {
  return <line className={className} x1={0} x2={0} y1={PAD.t} y2={BASE} style={{ transform: `translateX(${x}px)` }} />
}

/** Part des années au-dessus de `v`, interpolée dans la barre qui contient v. */
function shareAbove(s: Summary, v: number, total: number) {
  const pos = (v - BIN_MIN) / BIN_W
  const i = Math.min(s.bins.length - 1, Math.max(0, Math.floor(pos)))
  const above = s.bins.slice(i + 1).reduce((a, b) => a + b, 0) + s.bins[i] * (i + 1 - pos)
  return above / total
}

function FocusCard({ focus, s, total, style }: { focus: Focus; s: Summary; total: number; style: CSSProperties }) {
  const beats = shareAbove(s, SENIOR_RATE, total)
  return (
    <div className={`histo-focus ${focus}`} style={style} role="status">
      {focus === 'safe' ? (
        <>
          <p className="eyebrow">Safe</p>
          <p className="focus-big">{pct(SENIOR_RATE)}</p>
          <p>contract target rate, not guaranteed</p>
          <p>paid before Dynamic</p>
          <p className="muted">Dynamic beats it {Math.round(beats * 100)}% of years</p>
        </>
      ) : (
        <>
          <p className="eyebrow">Dynamic avg</p>
          <p className="focus-big">{pct(s.mean)}</p>
          <p>median {pct(s.median)}</p>
          <p>1 year in 20: {pct(s.p5, 0)}</p>
          <p className="muted">loses money {Math.round(s.loss * 100)}% of years</p>
        </>
      )}
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
