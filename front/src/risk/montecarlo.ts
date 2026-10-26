// [FRONT — Hugo] Monte-Carlo du pool de démarrage : portage de sim/tranches.py (small_pool), mêmes hypothèses.
// Un an = 4 échéances de 90 jours ; à chaque trimestre, n contrats de 10 k$ dont le sens est tiré au hasard
// (importateur avec la probabilité p), le peso bouge (normale + sauts), le pool encaisse spread et bons du Trésor,
// paie le non-compensé ; la cascade Safe (4,5 % fixe) / Dynamic (première perte) répartit le résultat de l'année.
// Générateur pseudo-aléatoire à graine (pas celui de numpy) : chiffres à ±1 point du tableau du README.

export const SENIOR_RATE = 0.045 // vault.rs : SENIOR_RATE_BPS = 450
export const CAP = 0.1 // plafond 10 %
const QUARTERS = 4
const JUMP = 0.15
const SPREAD_Q = 0.004 * 0.8 // 0,4 % par contrat de 90 j, dont 20 % pour le protocole
const TBILL = 0.04 // bons du Trésor tokenisés
const CONV = 0.003 // coût de conversion de la couverture en CETES, par an
const HEDGE_SHARE = 0.7 // part du Safe qui peut servir de couverture en CETES
const JUNIOR_UTIL = 0.15 // utilisation maximale du Dynamic
const CASH_SHARE = 0.08 // réserve en USDC
const SIZE = 10_000

export const SAFE = 400_000
export const DYNAMIC = 100_000

export type Params = {
  vol: number // vol annuelle du peso (0,13)
  importers: number // part d'importateurs (0,5)
  contracts: number // contrats par trimestre
  jumpP?: number // probabilité d'un saut de ±15 % par trimestre (0,0075)
  paths?: number
  seed?: number
}

export type Result = {
  dynamic: Float64Array // rendement annuel du Dynamic, une valeur par trajectoire
  netted: number // part du volume compensée
  pool: number // rendement moyen du pool
  mean: number // rendement moyen du Dynamic
  median: number
  loss: number // part des années perdantes
  p5: number // pire année sur 20
}

// xoshiro128** : rapide, à graine, assez bon pour 10^7 tirages.
function rng(seed: number) {
  let a = seed >>> 0 || 1
  let b = 0x9e3779b9
  let c = 0x243f6a88
  let d = 0xb7e15162
  const next = () => {
    const r = Math.imul(rotl(Math.imul(b, 5), 7), 9)
    const t = b << 9
    c ^= a
    d ^= b
    b ^= c
    a ^= d
    c ^= t
    d = rotl(d, 11)
    return (r >>> 0) / 4294967296
  }
  for (let i = 0; i < 16; i++) next()
  let spare: number | null = null
  const normal = () => {
    if (spare !== null) {
      const s = spare
      spare = null
      return s
    }
    const u = next() || 1e-12
    const v = next()
    const m = Math.sqrt(-2 * Math.log(u))
    spare = m * Math.sin(2 * Math.PI * v)
    return m * Math.cos(2 * Math.PI * v)
  }
  return { next, normal }
}
const rotl = (x: number, k: number) => (x << k) | (x >>> (32 - k))

export function simulate({ vol, importers: p, contracts: n, jumpP = 0.0075, paths = 40_000, seed = 3 }: Params): Result {
  const r = rng(seed)
  const tvl = SAFE + DYNAMIC
  const uCap = (JUNIOR_UTIL * DYNAMIC) / CAP // notionnel non couvert que le Dynamic peut porter
  const hCap = HEDGE_SHARE * SAFE // notionnel couvrable en CETES
  const volQ = vol * 0.5 // 90 jours : √0,25
  const gross = n * SIZE
  const cdf = binomialCdf(n, p)
  const dynamic = new Float64Array(paths)
  let pairedSum = 0
  let totSum = 0
  let losses = 0

  for (let i = 0; i < paths; i++) {
    let tot = 0
    for (let q = 0; q < QUARTERS; q++) {
      const imp = draw(cdf, r.next())
      const net = Math.abs(2 * imp - n) * SIZE
      const paired = gross - net
      const unhedged = Math.min(net, uCap)
      const hedged = Math.min(net - unhedged, hCap)
      const accepted = paired + unhedged + hedged
      pairedSum += gross ? paired / gross : 0
      const jump = r.next() < jumpP ? (r.next() < 0.5 ? -JUMP : JUMP) : 0
      const move = r.normal() * volQ + jump
      const invested = tvl + CAP * accepted - CASH_SHARE * tvl // dépôts + marges clients, hors réserve d'USDC
      tot +=
        (invested * TBILL) / 4 -
        (hedged * CONV) / 4 +
        accepted * SPREAD_Q -
        unhedged * Math.min(CAP, Math.max(-CAP, move)) +
        (jump !== 0 ? -hedged * (JUMP - CAP) * 0.5 : 0)
    }
    // Cascade : le Safe touche son taux en premier, le Dynamic absorbe le reste (perte bornée à son capital).
    const safe = Math.min(SAFE * SENIOR_RATE, tot + DYNAMIC)
    const dyn = Math.max(tot - safe, -DYNAMIC) / DYNAMIC
    dynamic[i] = dyn
    totSum += tot
    if (dyn < 0) losses++
  }

  const sorted = Float64Array.from(dynamic).sort()
  const mean = dynamic.reduce((s, x) => s + x, 0) / paths
  return {
    dynamic,
    netted: pairedSum / (paths * QUARTERS),
    pool: totSum / paths / tvl,
    mean,
    median: quantile(sorted, 0.5),
    loss: losses / paths,
    p5: quantile(sorted, 0.05),
  }
}

// Nombre d'importateurs ~ Binomiale(n, p), tiré par inversion de la fonction de répartition : un seul aléa
// par trimestre au lieu de n (exact, et 10 à 20 fois plus rapide pour 80 contrats).
function binomialCdf(n: number, p: number) {
  const cdf = new Float64Array(n + 1)
  let logPmf = n * Math.log(1 - p) // P(0), en log pour éviter les sous-dépassements
  let acc = 0
  for (let k = 0; k <= n; k++) {
    acc += p >= 1 ? (k === n ? 1 : 0) : Math.exp(logPmf)
    cdf[k] = acc
    logPmf += Math.log((n - k) / (k + 1)) + Math.log(p / (1 - p))
  }
  cdf[n] = Infinity // l'arrondi du cumul ne doit jamais laisser u au-delà du dernier palier
  return cdf
}

function draw(cdf: Float64Array, u: number) {
  let lo = 0
  let hi = cdf.length - 1
  while (lo < hi) {
    const mid = (lo + hi) >> 1
    if (cdf[mid] > u) hi = mid
    else lo = mid + 1
  }
  return lo
}

// Interpolation linéaire, comme np.percentile.
function quantile(sorted: Float64Array, q: number) {
  const pos = (sorted.length - 1) * q
  const lo = Math.floor(pos)
  const hi = Math.min(lo + 1, sorted.length - 1)
  return sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo)
}

// Histogramme du rendement Dynamic sur ±50 % par pas de 2,5 % ; les années au-delà vont dans les barres du bord.
export const BIN_MIN = -0.5
export const BIN_MAX = 0.5
export const BIN_W = 0.025

export type Summary = Omit<Result, 'dynamic'> & { bins: number[]; under: number; over: number; ms: number }

export function summarize(params: Params): Summary {
  const t = performance.now()
  const { dynamic, ...stats } = simulate(params)
  const n = Math.round((BIN_MAX - BIN_MIN) / BIN_W)
  const bins = new Array<number>(n).fill(0)
  let under = 0
  let over = 0
  for (const x of dynamic) {
    if (x < BIN_MIN) under++
    if (x >= BIN_MAX) over++
    bins[Math.min(n - 1, Math.max(0, Math.floor((x - BIN_MIN) / BIN_W)))]++
  }
  return { ...stats, bins, under, over, ms: performance.now() - t }
}
