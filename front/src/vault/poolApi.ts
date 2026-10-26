// [FRONT — Hugo] Adaptateur des appels LP du panneau « Safe / Dynamic » (tranches senior / junior).
// Seul ce fichier connaît les noms des fonctions du contrat : quand feat/tranches sera mergé (ou renommé),
// c'est ici qu'on ajuste. Tranches Safe/Dynamic déployées (deposit_senior, withdraw_senior, senior_balance, tranche_values) ;
// la tranche Safe se désactive seule face à un contrat qui ne les aurait pas.
import type { contract } from '@stellar/stellar-sdk'
import { pool, read, type PoolClient } from '../shared/contract'

/** Taux cible de la tranche Safe (SENIOR_RATE_BPS = 450 dans vault.rs), affiché seulement. Financé par la tranche Dynamic
 * tant que la trésorerie n'est pas placée (roadmap) : promis, pas gagné, et pas garanti si Dynamic est épuisée. */
export const SAFE_APR = 4.5
/** Part minimale de la tranche Dynamic dans les dépôts (MIN_JUNIOR_BPS = 2000). */
export const MIN_DYNAMIC_SHARE = 20

export type Kind = 'safe' | 'dynamic'
type Tx<T> = contract.AssembledTransaction<T>

interface TrancheValues {
  senior: bigint
  junior: bigint
  senior_price: bigint
}
// Tranches Safe/Dynamic déployées (deposit_senior, withdraw_senior, senior_balance, tranche_values).
interface TrancheMethods {
  deposit_senior(a: { lp: string; amount: bigint }): Promise<Tx<bigint>>
  withdraw_senior(a: { lp: string; shares: bigint }): Promise<Tx<bigint>>
  senior_balance(a: { lp: string }): Promise<Tx<bigint>>
  tranche_values(): Promise<Tx<TrancheValues>>
}
type Client = PoolClient & Partial<TrancheMethods>

const ONE = 10_000_000n

function hasTranches(p: Client): p is PoolClient & TrancheMethods {
  return typeof p.deposit_senior === 'function' && typeof p.tranche_values === 'function'
}

export interface Split {
  tranches: boolean
  total: bigint
  safe: bigint
  dynamic: bigint
  /** Capital bloqué par les couvertures en cours (pris sur la tranche Dynamic). */
  locked: bigint
  capacity: bigint
  safePrice: bigint
  sharePrice: bigint
  exitPrice: bigint
}

export async function readSplit(): Promise<Split> {
  const p = (await pool()) as Client
  const [total, capacity, sharePrice, exitPrice] = await Promise.all([
    read(p.total_assets()),
    read(p.capacity()),
    read(p.share_price()),
    read(p.exit_price()),
  ])
  const tv = hasTranches(p) ? await read(p.tranche_values()) : { senior: 0n, junior: total, senior_price: ONE }
  // capacité = dynamique − réservé  ⇒  réservé = dynamique − capacité
  const locked = tv.junior - capacity > 0n ? tv.junior - capacity : 0n
  return {
    tranches: hasTranches(p),
    total,
    safe: tv.senior,
    dynamic: tv.junior,
    locked,
    capacity,
    safePrice: tv.senior_price,
    sharePrice,
    exitPrice,
  }
}

export interface Mine {
  dynamic: { balance: bigint; shares: bigint }
  safe: { balance: bigint } | null
}

export async function readMine(lp: string): Promise<Mine> {
  const p = (await pool()) as Client
  const [balance, shares, safe] = await Promise.all([
    read(p.lp_balance({ lp })),
    read(p.lp_shares({ lp })),
    hasTranches(p) ? read(p.senior_balance({ lp })) : Promise.resolve(null),
  ])
  return { dynamic: { balance, shares }, safe: safe === null ? null : { balance: safe } }
}

// ---- écritures : chaque fonction construit la transaction, useTx la fait signer et l'envoie ----

export function deposit(kind: Kind, amount: bigint) {
  return (p: Client, lp: string): Promise<Tx<bigint>> => {
    if (kind === 'dynamic') return p.deposit_lp({ lp, amount })
    if (!hasTranches(p)) throw new Error('Safe deposits are not available yet.')
    return p.deposit_senior({ lp, amount })
  }
}

/**
 * Retrait d'un montant en USDC (`usdc`) ou de toute la position (`'all'`).
 * Le contrat prend des PARTS : on convertit avec le prix de sortie (Dynamic) ou le prix de la part Safe.
 */
export function withdraw(kind: Kind, usdc: bigint | 'all', split: Split, mine: Mine) {
  return (p: Client, lp: string): Promise<Tx<bigint>> => {
    if (kind === 'dynamic') {
      // exit_price est tronqué à 7 décimales : diviser par (prix + 1 unité) arrondit les parts vers le bas,
      // et on plafonne aux parts détenues (taper son solde affiché ne doit pas donner l'erreur 106).
      const calc = usdc === 'all' ? mine.dynamic.shares : (usdc * ONE) / (split.exitPrice + 1n)
      const shares = calc < mine.dynamic.shares ? calc : mine.dynamic.shares
      return p.withdraw({ lp, amount: shares })
    }
    if (!hasTranches(p)) throw new Error('Safe withdrawals are not available yet.')
    // Pas de lecture des parts Safe : on les déduit de la valeur. senior_price est TRONQUÉ à 7 décimales
    // (prix interne en 1e18) : diviser par lui seul arrondirait les parts vers le haut (erreur 112).
    // On divise par (prix + 1 unité) pour rester sous les parts détenues.
    const value = usdc === 'all' ? (mine.safe?.balance ?? 0n) : usdc
    return p.withdraw_senior({ lp, shares: (value * ONE) / (split.safePrice + 1n) })
  }
}
