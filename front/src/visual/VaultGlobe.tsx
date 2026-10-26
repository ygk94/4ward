// [FRONT] Petit vault du panneau pool (page Vaults), en Canvas 2D. Quand on choisit « Hedge an invoice »,
// il se transforme en globe de points (façon Ondo : terres en bleu foncé, côtes en bleu moyen, mer pâle),
// et un arc relie le dollar (New York) à la devise choisie : il se trace, un point le parcourt, le globe
// tourne pour centrer le trajet. L'arc va du dollar vers la devise quand la PME paie un fournisseur
// étranger, dans l'autre sens quand elle est payée en devise.
import { useEffect, useRef } from 'react'
import { isCoast, isLand } from './landMask'

/** Ville de chaque devise sur le globe (latitude, longitude). */
const CITIES: Record<string, [number, number]> = {
  USD: [40.7, -74.0], // New York
  MXN: [19.4, -99.1], // Mexico
  BRL: [-23.5, -46.6], // São Paulo
  ARS: [-34.6, -58.4], // Buenos Aires
  EUR: [50.1, 8.7], // Francfort
}

type V = [number, number, number]
const RAD = Math.PI / 180
const clamp01 = (x: number) => Math.min(1, Math.max(0, x))
const smooth = (x: number) => x * x * (3 - 2 * x)

/** Vecteur unitaire d'un point du globe (x vers la droite, y vers le haut, z vers nous à la longitude 0). */
const toVec = ([lat, lon]: [number, number]): V => [Math.cos(lat * RAD) * Math.sin(lon * RAD), Math.sin(lat * RAD), Math.cos(lat * RAD) * Math.cos(lon * RAD)]

// Petit vault : icosaèdre et dodécaèdre (sommets et arêtes, comme le vault de fond).
const PHI = (1 + Math.sqrt(5)) / 2
function solid(raw: V[], len: number, r: number) {
  const n = Math.hypot(...raw[0])
  const edges: [number, number][] = []
  for (let i = 0; i < raw.length; i++)
    for (let j = i + 1; j < raw.length; j++) {
      const d = Math.hypot(raw[i][0] - raw[j][0], raw[i][1] - raw[j][1], raw[i][2] - raw[j][2])
      if (Math.abs(d - len) < len * 0.05) edges.push([i, j])
    }
  return { verts: raw.map((v) => v.map((c) => (c / n) * r) as V), edges }
}
const ICO = solid(
  [[-1, PHI, 0], [1, PHI, 0], [-1, -PHI, 0], [1, -PHI, 0], [0, -1, PHI], [0, 1, PHI], [0, -1, -PHI], [0, 1, -PHI], [PHI, 0, -1], [PHI, 0, 1], [-PHI, 0, -1], [-PHI, 0, 1]],
  2,
  1,
)
const DODECA = (() => {
  const a = 1 / PHI
  const raw: V[] = []
  for (const x of [-1, 1]) for (const y of [-1, 1]) for (const z of [-1, 1]) raw.push([x, y, z])
  for (const s of [-1, 1]) for (const t of [-1, 1]) raw.push([0, s * a, t * PHI], [s * a, t * PHI, 0], [s * PHI, 0, t * a])
  return solid(raw, 2 * a, 0.55)
})()

function rotate(v: V, rx: number, ry: number): V {
  const [sx, cx, sy, cy] = [Math.sin(rx), Math.cos(rx), Math.sin(ry), Math.cos(ry)]
  const x = v[0] * cy + v[2] * sy
  const z = -v[0] * sy + v[2] * cy
  return [x, v[1] * cx - z * sx, v[1] * sx + z * cx]
}

interface Props {
  className?: string
  /** true : globe ; false : vault. */
  globe: boolean
  /** Devises reliées par l'arc (de → vers) ; absentes : pas d'arc. */
  from?: string
  to?: string
}

export function VaultGlobe({ className, globe, from, to }: Props) {
  const ref = useRef<HTMLCanvasElement>(null)
  const live = useRef({ globe, from, to })
  useEffect(() => {
    live.current = { globe, from, to }
  }, [globe, from, to])

  useEffect(() => {
    const canvas = ref.current!
    const ctx = canvas.getContext('2d')!
    const dpr = Math.min(window.devicePixelRatio || 1, 2)
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
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

    const jitter = ICO.edges.map(() => Array.from({ length: 4 }, () => Math.random() * Math.PI * 2))
    const start = performance.now()
    let last = 0
    let morph = live.current.globe ? 1 : 0
    let lon0 = -60 // longitude au centre du globe
    let lat0 = 15
    let linkKey = ''
    let drawn = 0 // avancement du tracé de l'arc (0 → 1)
    let raf = 0

    const frame = () => {
      raf = requestAnimationFrame(frame)
      if (!W || !H) return
      const t = ((performance.now() - start) / 1000) * (reduced ? 0.3 : 1)
      const dt = Math.min(0.1, t - last)
      last = t
      const { globe: wantGlobe, from: a, to: b } = live.current
      morph += ((wantGlobe ? 1 : 0) - morph) * (1 - Math.exp(-dt * 2.6))
      const m = smooth(clamp01(morph))

      const cx = W / 2
      const cy = H / 2
      const R = Math.min(W, H) * 0.46

      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      ctx.clearRect(0, 0, W, H)

      // --- Le vault : rétrécit et s'emballe pendant qu'il devient globe
      if (m < 0.98) {
        const k = 1 - m
        const s = R * (0.95 * k + 0.05)
        const spin = t * 0.5 + m * 6
        const rx = 0.35 + Math.sin(t * 0.4) * 0.2
        ctx.lineCap = 'round'
        const layers = [
          { color: '#2f6bff', width: 1.4, alpha: 0.9, amp: 0.006 },
          { color: '#6fd3ff', width: 0.8, alpha: 0.6, amp: 0.02 },
          { color: '#8c7bff', width: 0.6, alpha: 0.5, amp: 0.02 },
        ]
        const proj = (v: V) => [cx + v[0] * s, cy - v[1] * s, v[2]] as const
        for (const l of layers) {
          ctx.strokeStyle = l.color
          ctx.lineWidth = l.width
          ICO.edges.forEach(([i, j], e) => {
            const w = jitter[e]
            const p = rotate(ICO.verts[i], rx, spin)
            const q = rotate(ICO.verts[j], rx, spin)
            const A = proj([p[0] + Math.sin(t * 0.7 + w[0]) * l.amp, p[1] + Math.sin(t * 0.8 + w[1]) * l.amp, p[2]])
            const B = proj([q[0] + Math.sin(t * 0.75 + w[2]) * l.amp, q[1] + Math.sin(t * 0.65 + w[3]) * l.amp, q[2]])
            ctx.globalAlpha = l.alpha * k * (0.4 + 0.6 * clamp01(((A[2] + B[2]) / 2 + 1) / 2))
            ctx.beginPath()
            ctx.moveTo(A[0], A[1])
            ctx.lineTo(B[0], B[1])
            ctx.stroke()
          })
        }
        ctx.strokeStyle = '#8c7bff'
        ctx.lineWidth = 0.8
        DODECA.edges.forEach(([i, j]) => {
          const p = rotate(DODECA.verts[i], -t * 0.6, -t * 0.45)
          const q = rotate(DODECA.verts[j], -t * 0.6, -t * 0.45)
          ctx.globalAlpha = 0.55 * k
          ctx.beginPath()
          ctx.moveTo(cx + p[0] * s, cy - p[1] * s)
          ctx.lineTo(cx + q[0] * s, cy - q[1] * s)
          ctx.stroke()
        })
        ctx.fillStyle = '#2f6bff'
        ICO.verts.forEach((v) => {
          const p = rotate(v, rx, spin)
          ctx.globalAlpha = 0.9 * k * (0.3 + 0.7 * clamp01((p[2] + 1) / 2))
          ctx.beginPath()
          ctx.arc(cx + p[0] * s, cy - p[1] * s, 2.2, 0, Math.PI * 2)
          ctx.fill()
        })
      }

      if (m <= 0.02) return

      // --- Rotation du globe : il tourne doucement, ou se cale sur le milieu du trajet
      const A = a ? CITIES[a] : undefined
      const B = b ? CITIES[b] : undefined
      const key = A && B ? `${a}-${b}` : ''
      if (key !== linkKey) {
        linkKey = key
        drawn = 0
      }
      if (A && B) {
        const va = toVec(A)
        const vb = toVec(B)
        const mid: V = [va[0] + vb[0], va[1] + vb[1], va[2] + vb[2]]
        const tLon = Math.atan2(mid[0], mid[2]) / RAD
        const tLat = Math.max(-30, Math.min(30, Math.asin(mid[1] / Math.hypot(...mid)) / RAD))
        const dLon = ((tLon - lon0 + 540) % 360) - 180 // chemin le plus court
        lon0 += dLon * (1 - Math.exp(-dt * 2.2))
        lat0 += (tLat - lat0) * (1 - Math.exp(-dt * 2.2))
        if (Math.abs(dLon) < 25) drawn = Math.min(1, drawn + dt * 0.9)
      } else {
        lon0 -= dt * 8
        lat0 += (15 - lat0) * (1 - Math.exp(-dt))
      }
      const [sl, cl, sp, cp] = [Math.sin(lon0 * RAD), Math.cos(lon0 * RAD), Math.sin(lat0 * RAD), Math.cos(lat0 * RAD)]
      /** Globe → écran (x, y en pixels, z > 0 : face visible). */
      const view = (v: V, r = 1) => {
        const x = v[0] * cl - v[2] * sl
        const z1 = v[0] * sl + v[2] * cl
        const y = v[1] * cp - z1 * sp
        const z = v[1] * sp + z1 * cp
        return [cx + x * R * r, cy - y * R * r, z * r, Math.hypot(x, y) * r] as const
      }

      // --- Points du globe : grille régulière à l'écran, chaque point prend la couleur du lieu sous lui.
      // Ils apparaissent du centre vers le bord pendant la transformation.
      const ROWS = 30
      const g = (2 * R) / ROWS
      const reveal = m * 1.15
      for (let r = 0; r < ROWS; r++)
        for (let c = 0; c < ROWS; c++) {
          const x = (c + 0.5) / ROWS * 2 - 1
          const y = 1 - (r + 0.5) / ROWS * 2
          const rho = Math.hypot(x, y)
          if (rho > 1 || rho > reveal) continue
          const z = Math.sqrt(1 - rho * rho)
          const y1 = y * cp + z * sp
          const z1 = -y * sp + z * cp
          const px = x * cl + z1 * sl
          const pz = -x * sl + z1 * cl
          const lat = Math.asin(Math.max(-1, Math.min(1, y1))) / RAD
          const lon = Math.atan2(px, pz) / RAD
          const land = isLand(lat, lon)
          ctx.fillStyle = land ? '#2550d6' : isCoast(lat, lon) ? '#8fb0f5' : '#dbe6fb'
          ctx.globalAlpha = clamp01((reveal - rho) * 6) * (0.75 + 0.25 * z)
          ctx.beginPath()
          ctx.arc(cx + x * R, cy - y * R, g * 0.32, 0, Math.PI * 2)
          ctx.fill()
        }

      // --- Arc entre les deux villes : grand cercle soulevé au-dessus du globe
      if (A && B && drawn > 0) {
        const va = toVec(A)
        const vb = toVec(B)
        const omega = Math.acos(Math.max(-1, Math.min(1, va[0] * vb[0] + va[1] * vb[1] + va[2] * vb[2])))
        const lift = 0.12 + 0.3 * (omega / Math.PI)
        const N = 64
        const pts = Array.from({ length: N + 1 }, (_, i) => {
          const u = i / N
          const s1 = Math.sin((1 - u) * omega) / Math.sin(omega || 1e-6)
          const s2 = Math.sin(u * omega) / Math.sin(omega || 1e-6)
          const v: V = [va[0] * s1 + vb[0] * s2, va[1] * s1 + vb[1] * s2, va[2] * s1 + vb[2] * s2]
          return view(v, 1 + lift * Math.sin(Math.PI * u))
        })
        const shown = Math.round(N * smooth(drawn))
        const visible = (p: (typeof pts)[number]) => p[2] > 0 || p[3] > 1
        for (const [color, width, alpha] of [
          ['#6fd3ff', 6, 0.35],
          ['#2f6bff', 2, 1],
        ] as const) {
          ctx.strokeStyle = color
          ctx.lineWidth = width
          ctx.globalAlpha = alpha * m
          ctx.lineCap = 'round'
          ctx.beginPath()
          for (let i = 0; i < shown; i++) {
            if (!visible(pts[i]) || !visible(pts[i + 1])) continue
            ctx.moveTo(pts[i][0], pts[i][1])
            ctx.lineTo(pts[i + 1][0], pts[i + 1][1])
          }
          ctx.stroke()
        }
        // point qui parcourt l'arc une fois tracé
        if (drawn >= 1) {
          const p = pts[Math.floor(((t * 0.45) % 1) * N)]
          if (visible(p)) {
            ctx.globalAlpha = m
            ctx.fillStyle = '#ffffff'
            ctx.beginPath()
            ctx.arc(p[0], p[1], 3.2, 0, Math.PI * 2)
            ctx.fill()
            ctx.strokeStyle = '#2f6bff'
            ctx.lineWidth = 1.5
            ctx.stroke()
          }
        }
        // villes : point, anneau qui pulse, code de la devise
        ctx.font = "600 11px 'Space Grotesk', system-ui, sans-serif"
        ctx.textAlign = 'center'
        ;[
          [A, a!],
          [B, b!],
        ].forEach(([city, code], k) => {
          const p = view(toVec(city as [number, number]))
          if (p[2] <= 0) return
          const pulse = ((t * 0.8 + k * 0.5) % 1)
          ctx.globalAlpha = m * (1 - pulse)
          ctx.strokeStyle = '#2f6bff'
          ctx.lineWidth = 1.5
          ctx.beginPath()
          ctx.arc(p[0], p[1], 4 + pulse * 10, 0, Math.PI * 2)
          ctx.stroke()
          ctx.globalAlpha = m
          ctx.fillStyle = '#23252e'
          ctx.beginPath()
          ctx.arc(p[0], p[1], 3.5, 0, Math.PI * 2)
          ctx.fill()
          // étiquette sur une pastille crème pour rester lisible sur les points
          const label = code as string
          const tw = ctx.measureText(label).width + 10
          const ly = p[1] - 16
          ctx.fillStyle = 'rgba(251, 248, 241, 0.92)'
          ctx.beginPath()
          ctx.roundRect(p[0] - tw / 2, ly - 9, tw, 16, 5)
          ctx.fill()
          ctx.fillStyle = '#23252e'
          ctx.fillText(label, p[0], ly + 3)
        })
      }
      ctx.globalAlpha = 1
    }
    raf = requestAnimationFrame(frame)
    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
    }
  }, [])

  return <canvas ref={ref} className={`vault-globe ${className ?? ''}`} aria-hidden />
}
