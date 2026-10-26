// [FRONT] Vault de fond en Canvas 2D, à la manière du composant Framer « Polyhedron » : sommets projetés
// à la main, arêtes qui s'arrêtent net sur les sommets, un point sur chaque sommet, et l'arrière plus pâle
// que l'avant. On garde le vault à l'encre : icosaèdre en six passes de couleur qui tremblent, dodécaèdre
// violet qui tourne à contre-sens, cage géodésique, poussière, halo, traînée (le canvas s'efface un peu à
// chaque image au lieu d'être vidé).
// `burst` (0 → 1) : les arêtes éclatent en tournoyant, sans quitter l'écran ; onde de choc, rotation qui
// s'emballe. `chart` (0 → 1) : chaque arête rejoint une colonne d'un graphique en points sur une grille de
// points (façon Morpho) ; au survol, les colonnes proches de la souris se soulèvent et foncent.
// `rebuild` (avancement 0 → 1) : cinématique du Risk lab. Des éclats d'arête tombent du haut de l'écran et s'empilent
// dans chaque barre de l'histogramme (positions publiées dans burst.ts), puis s'effacent devant les vraies barres.
// Au changement d'onglet, les colonnes du graphique glissent dans le sens du menu, en retard sur la page (burst.slide).
import { useEffect, useRef } from 'react'
import { risk, SLIDE_MS, slide } from './burst'

type V = [number, number, number]
interface Shape {
  verts: V[]
  edges: [number, number][]
}

const PHI = (1 + Math.sqrt(5)) / 2
const dist = (a: V, b: V) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2])
const scale = (v: V, k: number): V => [v[0] * k, v[1] * k, v[2] * k]
const unit = (v: V): V => scale(v, 1 / Math.hypot(v[0], v[1], v[2]))
const mid = (a: V, b: V): V => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2]
const clamp01 = (x: number) => Math.min(1, Math.max(0, x))

/** Arêtes = paires de sommets à la distance voulue (comme le composant Framer). */
function edgesAt(verts: V[], len: number): [number, number][] {
  const out: [number, number][] = []
  for (let i = 0; i < verts.length; i++)
    for (let j = i + 1; j < verts.length; j++) if (Math.abs(dist(verts[i], verts[j]) - len) < len * 0.05) out.push([i, j])
  return out
}

function icosahedron(r: number): Shape {
  const raw: V[] = [
    [-1, PHI, 0], [1, PHI, 0], [-1, -PHI, 0], [1, -PHI, 0],
    [0, -1, PHI], [0, 1, PHI], [0, -1, -PHI], [0, 1, -PHI],
    [PHI, 0, -1], [PHI, 0, 1], [-PHI, 0, -1], [-PHI, 0, 1],
  ]
  return { verts: raw.map((v) => scale(unit(v), r)), edges: edgesAt(raw, 2) }
}

function dodecahedron(r: number): Shape {
  const a = 1 / PHI
  const raw: V[] = []
  for (const x of [-1, 1]) for (const y of [-1, 1]) for (const z of [-1, 1]) raw.push([x, y, z])
  for (const s of [-1, 1])
    for (const t of [-1, 1]) raw.push([0, s * a, t * PHI], [s * a, t * PHI, 0], [s * PHI, 0, t * a])
  return { verts: raw.map((v) => scale(v, r / Math.sqrt(3))), edges: edgesAt(raw, 2 * a) }
}

/** Icosaèdre subdivisé une fois (chaque face en quatre), sommets ramenés sur la sphère. */
function geodesic(r: number): Shape {
  const ico = icosahedron(1)
  const edgeKey = (i: number, j: number) => (i < j ? `${i}-${j}` : `${j}-${i}`)
  const isEdge = new Set(ico.edges.map(([i, j]) => edgeKey(i, j)))
  const verts = [...ico.verts]
  const index = new Map(verts.map((v, i) => [v.map((n) => n.toFixed(4)).join(), i]))
  const vid = (v: V) => {
    const k = v.map((n) => n.toFixed(4)).join()
    let i = index.get(k)
    if (i === undefined) {
      i = verts.length
      verts.push(v)
      index.set(k, i)
    }
    return i
  }
  const seen = new Set<string>()
  const edges: [number, number][] = []
  const add = (i: number, j: number) => {
    if (seen.has(edgeKey(i, j))) return
    seen.add(edgeKey(i, j))
    edges.push([i, j])
  }
  const n = ico.verts.length
  for (let i = 0; i < n; i++)
    for (let j = i + 1; j < n; j++)
      for (let k = j + 1; k < n; k++) {
        if (!isEdge.has(edgeKey(i, j)) || !isEdge.has(edgeKey(j, k)) || !isEdge.has(edgeKey(i, k))) continue
        const ab = vid(unit(mid(ico.verts[i], ico.verts[j])))
        const bc = vid(unit(mid(ico.verts[j], ico.verts[k])))
        const ca = vid(unit(mid(ico.verts[k], ico.verts[i])))
        const tri: [number, number][] = [[i, ab], [ab, j], [j, bc], [bc, k], [k, ca], [ca, i], [ab, bc], [bc, ca], [ca, ab]]
        tri.forEach(([p, q]) => add(p, q))
      }
  return { verts: verts.map((v) => scale(v, r)), edges }
}

/** Rotation d'Euler (ordre XYZ, comme three.js) précalculée pour une image. */
function rotator(rx: number, ry: number, rz: number) {
  const [sx, cx, sy, cy, sz, cz] = [Math.sin(rx), Math.cos(rx), Math.sin(ry), Math.cos(ry), Math.sin(rz), Math.cos(rz)]
  return (v: V): V => {
    const x1 = v[0] * cz - v[1] * sz
    const y1 = v[0] * sz + v[1] * cz
    const x2 = x1 * cy + v[2] * sy
    const z2 = -x1 * sy + v[2] * cy
    return [x2, y1 * cx - z2 * sx, y1 * sx + z2 * cx]
  }
}

/** Rotation de p autour de l'axe unitaire k (formule de Rodrigues). */
function aroundAxis(p: V, k: V, angle: number): V {
  const c = Math.cos(angle)
  const s = Math.sin(angle)
  const dot = (k[0] * p[0] + k[1] * p[1] + k[2] * p[2]) * (1 - c)
  return [
    p[0] * c + (k[1] * p[2] - k[2] * p[1]) * s + k[0] * dot,
    p[1] * c + (k[2] * p[0] - k[0] * p[2]) * s + k[1] * dot,
    p[2] * c + (k[0] * p[1] - k[1] * p[0]) * s + k[2] * dot,
  ]
}

const randomDir = (): V => {
  const u = Math.random() * 2 - 1
  const a = Math.random() * Math.PI * 2
  const r = Math.sqrt(1 - u * u)
  return [r * Math.cos(a), r * Math.sin(a), u]
}

/** Trajectoire d'éclatement d'une arête : direction depuis le centre, distance, axe et vitesse de rotation. */
interface Fly {
  dir: V
  dist: number
  axis: V
  spin: number
}

function makeFlies(shape: Shape, reach: number): Fly[] {
  return shape.edges.map(([i, j]) => {
    const m = mid(shape.verts[i], shape.verts[j])
    const rnd = scale(randomDir(), 0.6)
    const len = Math.hypot(m[0], m[1], m[2])
    const dir = unit(len < 1e-6 ? rnd : [m[0] / len + rnd[0], m[1] / len + rnd[1], m[2] / len + rnd[2]])
    return { dir, dist: reach * (0.7 + Math.random() * 0.8), axis: randomDir(), spin: (Math.random() - 0.5) * 6 }
  })
}

interface Layer {
  color: string
  width: number
  opacity: number
  jitter: Float32Array
}

/** Une forme : ses passes de couleur, ses trajectoires d'éclatement et la colonne de chaque arête. */
interface Part {
  shape: Shape
  layers: Layer[]
  flies: Fly[]
  amp: number
  cols: Int32Array
  radius: number
  dot?: { color: string; size: number }
  a: V[]
  b: V[]
}

function makePart(
  shape: Shape,
  layers: Omit<Layer, 'jitter'>[],
  reach: number,
  amp: number,
  radius: number,
  dot?: Part['dot'],
): Part {
  const n = shape.edges.length
  return {
    shape,
    layers: layers.map((l) => ({ ...l, jitter: new Float32Array(n * 6).map(() => Math.random() * Math.PI * 2) })),
    flies: makeFlies(shape, reach),
    amp,
    cols: new Int32Array(n),
    radius,
    dot,
    a: shape.edges.map(() => [0, 0, 0] as V),
    b: shape.edges.map(() => [0, 0, 0] as V),
  }
}

const BLUE: V = [47, 107, 255]
const HOT: V = [27, 47, 168] // colonnes survolées : bleu plus profond

interface Props {
  className?: string
  burst?: () => number
  chart?: () => number
  rebuild?: () => number
}

/** Passes de couleur des éclats du Risk lab : gains en bleu, pertes en violet (comme les barres). */
const SHARD_LAYERS = {
  gain: [
    { color: '#2f6bff', width: 2.2, opacity: 0.95 },
    { color: '#6fd3ff', width: 1, opacity: 0.6 },
    { color: '#6fd3ff', width: 4, opacity: 0.12 },
  ],
  loss: [
    { color: '#8c7bff', width: 2.2, opacity: 0.95 },
    { color: '#b9a8ff', width: 1, opacity: 0.6 },
    { color: '#8c7bff', width: 4, opacity: 0.12 },
  ],
}
/** Colonnes de points par barre (même grille que Histogram.tsx) et nombre maximal d'éclats par couleur. */
const SHARD_COLS = 4
const SH = 900

export function PolyVault({ className, burst, chart, rebuild }: Props) {
  const ref = useRef<HTMLCanvasElement>(null)
  const burstRef = useRef(burst)
  const chartRef = useRef(chart)
  const rebuildRef = useRef(rebuild)
  useEffect(() => {
    burstRef.current = burst
    chartRef.current = chart
    rebuildRef.current = rebuild
  }, [burst, chart, rebuild])

  useEffect(() => {
    const canvas = ref.current!
    const ctx = canvas.getContext('2d')!
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const dpr = Math.min(window.devicePixelRatio || 1, 2)

    const outer = makePart(
      icosahedron(1.75),
      [
        { color: '#2f6bff', width: 1.4, opacity: 0.9 },
        { color: '#6fd3ff', width: 0.8, opacity: 0.6 },
        { color: '#6fd3ff', width: 3, opacity: 0.1 },
        { color: '#8c7bff', width: 0.6, opacity: 0.5 },
        { color: '#b9f3ff', width: 0.5, opacity: 0.4 },
        { color: '#8c7bff', width: 0.4, opacity: 0.35 },
      ],
      1.9,
      0.03,
      1.75,
      { color: '#2f6bff', size: 2.6 },
    )
    const inner = makePart(
      dodecahedron(0.95),
      [
        { color: '#8c7bff', width: 0.9, opacity: 0.6 },
        { color: '#8c7bff', width: 0.5, opacity: 0.4 },
      ],
      1.5,
      0.025,
      0.95,
      { color: '#8c7bff', size: 1.8 },
    )
    const cage = makePart(
      geodesic(2.35),
      [
        { color: '#6fd3ff', width: 0.7, opacity: 0.28 },
        { color: '#2f6bff', width: 0.4, opacity: 0.16 },
      ],
      2.4,
      0.02,
      2.35,
    )
    const parts = [outer, inner, cage]

    // Graphique : une colonne par arête, dans un ordre mélangé pour que les trajectoires se croisent ;
    // hauteurs tirées d'une marche aléatoire qui monte, normalisée entre 0,15 et 1.
    const NB = parts.reduce((n, p) => n + p.shape.edges.length, 0)
    const series = new Float32Array(NB)
    let walk = 0
    for (let i = 0; i < NB; i++) series[i] = walk += (Math.random() - 0.4) * 0.12
    const [lo, hi] = [Math.min(...series), Math.max(...series)]
    series.forEach((h, i) => (series[i] = 0.15 + (0.85 * (h - lo)) / (hi - lo || 1)))
    const perm = Array.from({ length: NB }, (_, i) => i).sort(() => Math.random() - 0.5)
    let from = 0
    for (const p of parts) {
      p.cols = Int32Array.from(perm.slice(from, from + p.shape.edges.length))
      from += p.shape.edges.length
    }
    const colX = new Float32Array(NB)
    const colTop = new Float32Array(NB)
    const lift = new Float32Array(NB)
    const ROWS = 70
    const COL_DOTS = Math.ceil(ROWS * 0.62)

    // Poussière autour du vault : part vers l'extérieur à l'explosion (sans quitter l'écran).
    const DUST = 900
    const dust = Array.from({ length: DUST }, () => ({
      base: scale(randomDir(), 2.1 + Math.random() * 1.6),
      reach: 0.5 + Math.random() * 1.1,
    }))

    let W = 0
    let H = 0
    const resize = () => {
      const r = canvas.getBoundingClientRect()
      W = r.width
      H = r.height
      canvas.width = Math.round(W * dpr)
      canvas.height = Math.round(H * dpr)
    }
    const ro = new ResizeObserver(resize)
    ro.observe(canvas)
    resize()

    const pointer = { x: 0, y: 0, inside: false }
    const onMove = (e: PointerEvent) => {
      pointer.x = e.clientX / window.innerWidth - 0.5
      pointer.y = e.clientY / window.innerHeight - 0.5
      pointer.inside = true
    }
    const onLeave = () => (pointer.inside = false)
    window.addEventListener('pointermove', onMove)
    document.addEventListener('pointerleave', onLeave)

    // Éclats du Risk lab : tirages fixes par éclat (retard, dérive) et tremblement de chaque passe.
    const shardRnd = Float32Array.from({ length: SH * 2 }, () => Math.random())
    const shardJitter = Float32Array.from({ length: SH * 4 }, () => Math.random() * Math.PI * 2)

    // Changement d'onglet : les colonnes partent du côté d'où arrive la page, en retard (les colonnes en tête d'abord),
    // dépassent un peu leur place puis s'y calent exactement.
    const slideOff = (i: number, n: number, w: number) => {
      if (!slide.dir) return 0
      const age = (performance.now() - slide.start) / SLIDE_MS
      if (age >= 1) return 0
      const delay = 0.15 + (slide.dir > 0 ? 1 - i / n : i / n) * 0.3 // la page est déjà là : les colonnes arrivent après
      const u = Math.min(1, Math.max(0, (age - delay) / 0.55))
      const back = 1 + 2.2 * (u - 1) ** 3 + 1.2 * (u - 1) ** 2 // easeOutBack : 0 → 1 avec un léger dépassement
      return -slide.dir * w * 0.1 * (1 - back)
    }

    const outerPhase = outer.shape.verts.map(() => Math.random() * Math.PI * 2)
    const innerPhase = inner.shape.verts.map(() => Math.random() * Math.PI * 2)
    const start = performance.now()
    let raf = 0
    let blast = 1 // le vault arrive en morceaux et s'assemble
    let spin = 0
    let last = 0
    let morph = 0
    let hover = 0

    const frame = () => {
      raf = requestAnimationFrame(frame)
      if (!W || !H) return
      const t = ((performance.now() - start) / 1000) * (reduced ? 0.3 : 1)
      const dt = Math.min(0.1, t - last)
      last = t

      // éclatement et graphique lissés, indépendants du nombre d'images par seconde
      const target = clamp01(burstRef.current?.() ?? 0)
      const opening = target > blast
      blast += (target - blast) * (1 - Math.exp(-dt * 4.5))
      const eased = blast * blast * (3 - 2 * blast)
      // la rotation s'emballe quand le vault éclate, beaucoup moins quand il se reforme
      spin += dt * (0.18 + eased * (opening ? 2.2 : 0.45))
      morph += (clamp01(chartRef.current?.() ?? 0) - morph) * (1 - Math.exp(-dt * 1.6))
      const ce = morph * morph * (3 - 2 * morph)
      const still = 1 - ce // en graphique, les formes cessent de tourner
      hover += ((pointer.inside ? 1 : 0) - hover) * (1 - Math.exp(-dt * 3))

      // Caméra : même cadrage que l'ancien vault WebGL (champ vertical 32°, recul en portrait).
      const D = 8 * Math.max(1, H / W)
      const f = H / 2 / Math.tan((16 * Math.PI) / 180)
      const cx = W / 2
      const cy = H / 2
      const project = (v: V) => {
        const s = f / Math.max(0.1, D - v[2])
        return [cx + v[0] * s, cy - v[1] * s] as const
      }
      const hWorld = (H / f) * D // hauteur visible dans le plan z = 0
      const wWorld = hWorld * (W / H)
      const bottom = -hWorld / 2 + hWorld * 0.04

      // colonnes du graphique, avec la bosse du survol autour de la souris
      const px = pointer.x * wWorld
      const reachX = wWorld * 0.07
      for (let i = 0; i < NB; i++) {
        colX[i] = (-wWorld / 2 + ((i + 0.5) / NB) * wWorld) * 0.96 + slideOff(i, NB, wWorld)
        lift[i] = hover * Math.exp(-(((colX[i] - px) / reachX) ** 2))
        colTop[i] = bottom + hWorld * 0.56 * series[i] * (1 + 0.05 * Math.sin(t * 0.9 + i * 0.35)) + hWorld * 0.14 * lift[i]
      }

      const rotations = [
        rotator(
          (0.35 + Math.sin(t * 0.21) * 0.25 + pointer.y * 0.4) * still,
          (spin + pointer.x * 0.6) * still,
          Math.sin(t * 0.13) * 0.15 * still,
        ),
        rotator(-t * 0.27 * still, -t * 0.22 * still, t * 0.1 * still),
        rotator((-0.2 + pointer.y * 0.2) * still, (-spin * 0.4 + pointer.x * 0.3) * still, 0.1 * still),
      ]

      // Respiration : chaque sommet s'écarte un peu du centre, à son rythme.
      const local = [
        outer.shape.verts.map((v, i) => scale(v, 1 + 0.06 * Math.sin(t * 0.55 + outerPhase[i]))),
        inner.shape.verts.map((v, i) => scale(v, 1 + 0.12 * Math.sin(t * 0.8 + innerPhase[i]))),
        cage.shape.verts,
      ]

      // Extrémités de chaque arête dans la scène : éclatement, rotation, puis colonne du graphique.
      parts.forEach((p, k) => {
        const rot = rotations[k]
        p.shape.edges.forEach(([i, j], e) => {
          let a = local[k][i]
          let b = local[k][j]
          if (eased > 0) {
            // l'arête tourne sur son milieu, puis le milieu part vers l'extérieur
            const fl = p.flies[e]
            const m = mid(a, b)
            const half: V = [a[0] - m[0], a[1] - m[1], a[2] - m[2]]
            const r = aroundAxis(half, fl.axis, fl.spin * eased)
            const c: V = [m[0] + fl.dir[0] * fl.dist * eased, m[1] + fl.dir[1] * fl.dist * eased, m[2] + fl.dir[2] * fl.dist * eased]
            a = [c[0] + r[0], c[1] + r[1], c[2] + r[2]]
            b = [c[0] - r[0], c[1] - r[1], c[2] - r[2]]
          }
          a = rot(a)
          b = rot(b)
          if (ce > 0) {
            const n = p.cols[e]
            a = [a[0] + (colX[n] - a[0]) * ce, a[1] + (bottom - a[1]) * ce, a[2] * (1 - ce)]
            b = [b[0] + (colX[n] - b[0]) * ce, b[1] + (colTop[n] - b[1]) * ce, b[2] * (1 - ce)]
          }
          p.a[e] = a
          p.b[e] = b
        })
      })

      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      // traînée d'encre : on efface 20 % de l'image précédente au lieu de tout vider
      ctx.globalCompositeOperation = 'destination-out'
      ctx.globalAlpha = 1
      ctx.fillStyle = 'rgba(0, 0, 0, 0.2)'
      ctx.fillRect(0, 0, W, H)
      ctx.globalCompositeOperation = 'source-over'

      // Halo au centre
      const unitPx = f / D
      const pulse = 1 + 0.08 * Math.sin(t * 1.3)
      const glowR = ((2.6 + eased * 5) * pulse * unitPx) / 2
      const glowA = 0.09 * (1 - 0.5 * eased) * still
      if (glowA > 0.002) {
        const g = ctx.createRadialGradient(cx, cy, 0, cx, cy, glowR)
        g.addColorStop(0, 'rgba(111, 211, 255, 1)')
        g.addColorStop(0.35, 'rgba(47, 107, 255, 0.45)')
        g.addColorStop(1, 'rgba(140, 123, 255, 0)')
        ctx.globalAlpha = glowA
        ctx.fillStyle = g
        ctx.fillRect(cx - glowR, cy - glowR, glowR * 2, glowR * 2)
      }

      // Poussière
      const dustA = 0.5 * (1 - 0.6 * eased) * still
      if (dustA > 0.002) {
        const rot = rotator(t * 0.03, spin * 0.25, 0)
        ctx.globalAlpha = dustA
        ctx.fillStyle = '#2f6bff'
        ctx.beginPath()
        dust.forEach((d, i) => {
          const k = 1 + eased * d.reach + 0.04 * Math.sin(t * 0.7 + i)
          const [x, y] = project(rot(scale(d.base, k)))
          ctx.rect(x - 1, y - 1, 2, 2)
        })
        ctx.fill()
      }

      // Arêtes : chaque passe de couleur tremble à son rythme ; l'arrière est plus pâle que l'avant.
      const fade = clamp01((ce - 0.7) / 0.3)
      ctx.lineCap = 'round'
      for (const p of parts) {
        p.layers.forEach((l, k) => {
          const ampK = k === 0 ? p.amp * 0.3 : p.amp
          const base = l.opacity * (1 - 0.75 * eased) * (1 - 0.8 * fade)
          if (base < 0.002) return
          ctx.strokeStyle = l.color
          ctx.lineWidth = l.width
          for (let e = 0; e < p.a.length; e++) {
            const o = e * 6
            const j = (c: number) => Math.sin(t * (0.6 + (c % 3) * 0.17) + l.jitter[o + c]) * ampK
            const a: V = [p.a[e][0] + j(0), p.a[e][1] + j(1), p.a[e][2] + j(2)]
            const b: V = [p.b[e][0] + j(3), p.b[e][1] + j(4), p.b[e][2] + j(5)]
            const depth = clamp01(((a[2] + b[2]) / 2 + p.radius) / (2 * p.radius))
            ctx.globalAlpha = base * (0.35 + 0.65 * depth + (0.65 - 0.65 * depth) * ce)
            const [ax, ay] = project(a)
            const [bx, by] = project(b)
            ctx.beginPath()
            ctx.moveTo(ax, ay)
            ctx.lineTo(bx, by)
            ctx.stroke()
          }
        })
      }

      // Points sur les sommets (comme le composant Framer), tant que le vault est assemblé.
      const dotA = (1 - eased) * still
      if (dotA > 0.01)
        [outer, inner].forEach((p, k) => {
          if (!p.dot) return
          ctx.fillStyle = p.dot.color
          local[k].forEach((v) => {
            const w = rotations[k](v)
            const depth = clamp01((w[2] + p.radius) / (2 * p.radius))
            const [x, y] = project(w)
            ctx.globalAlpha = dotA * 0.9 * (0.25 + 0.75 * depth)
            ctx.beginPath()
            ctx.arc(x, y, p.dot!.size, 0, Math.PI * 2)
            ctx.fill()
          })
        })

      // Onde de choc pendant l'explosion
      const ringA = Math.sin(Math.PI * eased) * 0.55 * still
      if (ringA > 0.01) {
        ctx.globalAlpha = ringA
        ctx.strokeStyle = '#6fd3ff'
        ctx.lineWidth = 2.2
        ctx.beginPath()
        ctx.arc(cx, cy, (0.4 + eased * 5.5) * unitPx, 0, Math.PI * 2)
        ctx.stroke()
      }

      // Graphique en points : grille pâle sur tout l'écran, colonnes en bleu.
      const dots = clamp01((ce - 0.55) / 0.45)
      if (dots > 0.002) {
        const dy = hWorld / ROWS
        const amp = hWorld * 0.0012 // les points tremblent comme l'encre
        const toX = (x: number) => cx + x * unitPx
        const toY = (y: number) => cy - y * unitPx
        ctx.globalAlpha = 0.14 * dots
        ctx.fillStyle = '#23252e'
        ctx.beginPath()
        for (let i = 0; i < NB; i++)
          for (let r = 0; r < ROWS; r++) {
            const k = i * ROWS + r
            ctx.rect(toX(colX[i] + Math.sin(t * 9 + k * 1.7) * amp) - 0.8, toY(-hWorld / 2 + (r + 0.5) * dy + Math.cos(t * 7.3 + k * 2.3) * amp) - 0.8, 1.6, 1.6)
          }
        ctx.fill()
        ctx.globalAlpha = 0.75 * dots
        for (let i = 0; i < NB; i++) {
          const c = BLUE.map((v, n) => Math.round(v + (HOT[n] - v) * lift[i]))
          ctx.fillStyle = `rgb(${c[0]}, ${c[1]}, ${c[2]})`
          ctx.beginPath()
          for (let r = 0; r < COL_DOTS; r++) {
            const y = bottom + r * dy
            if (y > colTop[i]) break
            const k = i * COL_DOTS + r
            ctx.rect(toX(colX[i] + Math.sin(t * 11 + k * 1.3) * amp) - 1.2, toY(y + Math.cos(t * 8.1 + k * 1.9) * amp) - 1.2, 2.4, 2.4)
          }
          ctx.fill()
        }
      }

      // Cinématique du Risk lab, en pixels : les barres sont publiées en fractions du viewport (le canvas le couvre).
      const rbP = rebuildRef.current?.()
      if (rbP !== undefined && rbP < 1 && risk.rects.length) {
        const fadeOut = 1 - clamp01((rbP - 0.78) / 0.17) // les vraies barres (points) prennent le relais
        const counts = { gain: 0, loss: 0 }
        const poses: Record<'gain' | 'loss', number[]> = { gain: [], loss: [] }
        risk.rects.forEach((r, bin) => {
          const x0 = r.x0 * W
          const x1 = r.x1 * W
          const slot = (x1 - x0) / SHARD_COLS
          if (slot <= 0) return
          const rows = Math.floor(((r.bottom - r.top) * H) / slot + 1e-6)
          const kind = bin < risk.rects.length / 2 ? 'loss' : 'gain'
          for (let row = 0; row < rows; row++) {
            const k = counts[kind]++
            if (k >= SH) continue
            const [r1, r2] = [shardRnd[k * 2], shardRnd[k * 2 + 1]]
            // les rangées du bas tombent d'abord : la barre se remplit de bas en haut
            const delay = (row / rows) * 0.45 + r1 * 0.12
            const fall = clamp01((rbP - delay * 0.8) / 0.3)
            if (fall <= 0) continue
            // chute qui accélère puis ralentit à l'arrivée (l'éclat se pose sans à-coup)
            const e = 0.35 * fall * fall + 0.65 * fall * fall * (3 - 2 * fall)
            const ty = r.bottom * H - (row + 0.5) * slot
            const startY = -slot * 2 - r1 * H * 0.3
            poses[kind].push(
              k,
              (x0 + x1) / 2 + (1 - e) * (r2 - 0.5) * slot * 4,
              startY + (ty - startY) * e,
              (1 - e) * (1 - e) * (r2 - 0.5) * 8, // tournoie, puis se redresse en douceur
              ((x1 - x0) / 2) * 0.88 * (1 + (1 - e) * 0.6),
            )
          }
        })
        for (const kind of ['loss', 'gain'] as const) {
          const pose = poses[kind]
          SHARD_LAYERS[kind].forEach((l, layer) => {
            ctx.globalAlpha = l.opacity * fadeOut
            ctx.strokeStyle = l.color
            ctx.lineWidth = l.width
            const amp = layer === 0 ? 0.6 : 1.8
            ctx.beginPath()
            for (let n = 0; n < pose.length; n += 5) {
              const [k, sx, sy, ang, len] = [pose[n], pose[n + 1], pose[n + 2], pose[n + 3], pose[n + 4]]
              const jx = Math.sin(t * 0.6 + shardJitter[k * 4 + layer]) * amp
              const jy = Math.cos(t * 0.77 + shardJitter[k * 4 + layer]) * amp
              const [ex, ey] = [Math.cos(ang) * len, Math.sin(ang) * len]
              ctx.moveTo(sx - ex + jx, sy - ey + jy)
              ctx.lineTo(sx + ex + jx, sy + ey + jy)
            }
            ctx.stroke()
          })
        }
      }
      ctx.globalAlpha = 1
    }
    raf = requestAnimationFrame(frame)

    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
      window.removeEventListener('pointermove', onMove)
      document.removeEventListener('pointerleave', onLeave)
    }
  }, [])

  return <canvas ref={ref} className={`poly-vault ${className ?? ''}`} aria-hidden />
}
