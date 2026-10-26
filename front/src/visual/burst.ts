// [FRONT] Éclatement du vault de fond (0 = assemblé, 1 = en morceaux).
// L'accueil ne défile pas (il capte la molette) : il pousse sa propre progression dans `burst.home`.
// Sur les autres pages, le vault reste ouvert et devient un graphique (cf. Background).
export const burst = { home: 0 }

export const homeBurst = () => burst.home
export const opened = () => 1
export const closed = () => 0
