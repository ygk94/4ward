// [FRONT] Éclatement du vault de fond (0 = assemblé, 1 = en morceaux).
// L'accueil ne défile pas (il capte la molette) : il pousse sa propre progression dans `burst.home`.
// Sur les autres pages, le vault reste ouvert et devient un graphique (cf. Background).
export const burst = { home: 0 }

export const homeBurst = () => burst.home
export const opened = () => 1
export const closed = () => 0

// [FRONT] Cinématique du Risk lab : les arêtes du vault remontent du bas de l'écran pour refaire l'histogramme,
// puis une partie d'entre elles se décompose en colonnes de points (le fond des autres pages).
// L'histogramme publie ici la position de ses barres (fractions du viewport) ; la cinématique démarre à la première publication.
export interface BinRect {
  x0: number
  x1: number
  top: number
  bottom: number
}
export const RISK_MS = 3000
// [FRONT] Changement d'onglet : la page glisse dans le sens du menu (dir = +1 en avançant, −1 en reculant, 0 sinon) ;
// les billes du fond (colonnes de points) suivent en retard puis se calent (cf. InkVault, colX).
export const SLIDE_MS = 2600
export const slide = { dir: 0, start: 0 }

export const risk = { rects: [] as BinRect[], start: 0 }

const reduced = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches
/** Avancement 0 → 1 de la cinématique (1 d'emblée si l'utilisateur limite les animations). */
export const riskProgress = () => {
  if (reduced()) return 1
  return risk.start ? Math.min(1, (performance.now() - risk.start) / RISK_MS) : 0
}
