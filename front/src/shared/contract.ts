// [FRONT — Hugo] Client du contrat pool.
// Le client est construit à partir de la spec lue on-chain (contract.Client.from) : pas besoin de régénérer
// les bindings à chaque redéploiement. Les types ci-dessous reprennent ceux de contracts/pool/src/lib.rs.
import { contract } from '@stellar/stellar-sdk'
import { config } from './config'
import { signTransaction } from './wallet'

export type SideTag = 'BuyCurrency' | 'SellCurrency'

export interface Quote {
  spot: bigint
  forward_rate: bigint
  usdc_cost: bigint
  margin: bigint
}

export interface Hedge {
  owner: string
  supplier: string
  currency: string
  side: { tag: SideTag }
  notional: bigint
  locked_rate: bigint
  cap_bps: number
  margin: bigint
  maturity: bigint
  settled: boolean
}

export interface Exposure {
  buy_notional: bigint
  sell_notional: bigint
  net_notional: bigint
  buy_margin: bigint
  sell_margin: bigint
}

export interface PoolStats {
  totalAssets: bigint
  capacity: bigint
  sharePrice: bigint
  exitPrice: bigint
  exposure: Exposure
}

type Tx<T> = contract.AssembledTransaction<T>
// Méthodes du pool (signatures de lib.rs, arguments nommés comme en Rust).
interface PoolMethods {
  total_assets(): Promise<Tx<bigint>>
  capacity(): Promise<Tx<bigint>>
  share_price(): Promise<Tx<bigint>>
  exit_price(): Promise<Tx<bigint>>
  /** Facteur d'accélération du temps (mode démo : 25920, 90 jours = 5 min réelles). */
  time_factor(): Promise<Tx<number>>
  lp_balance(a: { lp: string }): Promise<Tx<bigint>>
  lp_shares(a: { lp: string }): Promise<Tx<bigint>>
  exposure(a: { currency: string }): Promise<Tx<Exposure>>
  quote(a: { currency: string; side: { tag: SideTag }; notional: bigint; duration_secs: bigint; cap_bps: number }): Promise<Tx<Quote>>
  deposit_lp(a: { lp: string; amount: bigint }): Promise<Tx<bigint>>
  withdraw(a: { lp: string; amount: bigint }): Promise<Tx<bigint>>
  open_hedge(a: {
    owner: string
    supplier: string
    currency: string
    side: { tag: SideTag }
    notional: bigint
    duration_secs: bigint
    cap_bps: number
  }): Promise<Tx<bigint>>
  settle(a: { hedge_id: bigint }): Promise<Tx<bigint>>
  close_early(a: { hedge_id: bigint }): Promise<Tx<bigint>>
  get_hedge(a: { hedge_id: bigint }): Promise<Tx<Hedge | undefined>>
}
export type PoolClient = contract.Client & PoolMethods

const clients = new Map<string, Promise<PoolClient>>()

/** Client du pool. Avec `publicKey`, les écritures sont signées par le wallet connecté. */
export function pool(publicKey?: string): Promise<PoolClient> {
  const key = publicKey ?? ''
  let c = clients.get(key)
  if (!c) {
    c = contract.Client.from({
      contractId: config.poolId,
      rpcUrl: config.rpcUrl,
      networkPassphrase: config.networkPassphrase,
      publicKey,
      signTransaction: publicKey ? signTransaction : undefined,
    }) as Promise<PoolClient>
    c.catch(() => clients.delete(key))
    clients.set(key, c)
  }
  return c
}

/** Erreur métier du contrat. Le SDK ne donne que la doc du variant (en français) : on retrouve le code à partir d'elle. */
export class ContractError extends Error {
  readonly doc: string
  constructor(doc: string) {
    super(doc)
    this.doc = doc
  }
}

interface ResultLike<T> {
  isErr(): boolean
  unwrap(): T
  error: { message: string }
}

/** Le SDK renvoie les erreurs du contrat comme un `Err` (et parfois les succès comme un `Ok`) : on déballe. */
export function unwrap<T>(r: T | ResultLike<T>): T {
  if (r && typeof r === 'object' && typeof (r as ResultLike<T>).isErr === 'function') {
    const res = r as ResultLike<T>
    if (res.isErr()) throw new ContractError(res.error.message)
    return res.unwrap()
  }
  return r as T
}

/** Simule un appel en lecture et retourne sa valeur. */
export async function read<T>(tx: Promise<Tx<T>>): Promise<T> {
  return unwrap((await tx).result)
}

export async function readStats(currency = 'MXN'): Promise<PoolStats> {
  const p = await pool()
  const [totalAssets, capacity, sharePrice, exitPrice, exposure] = await Promise.all([
    read(p.total_assets()),
    read(p.capacity()),
    read(p.share_price()),
    read(p.exit_price()),
    read(p.exposure({ currency })),
  ])
  return { totalAssets, capacity, sharePrice, exitPrice, exposure }
}

/** Couvertures d'un propriétaire. Le contrat n'indexe pas par owner : on parcourt les ids jusqu'au premier vide. */
export async function readHedges(owner: string, max = 60): Promise<{ id: bigint; hedge: Hedge }[]> {
  const p = await pool()
  const out: { id: bigint; hedge: Hedge }[] = []
  for (let start = 0n; start < BigInt(max); start += 6n) { // le contrat numérote à partir de 0 (NextId = 0)
    const ids = Array.from({ length: 6 }, (_, i) => start + BigInt(i))
    const batch = await Promise.all(ids.map((id) => read(p.get_hedge({ hedge_id: id }))))
    let done = false
    batch.forEach((h, i) => {
      if (!h) done = true
      else if (h.owner === owner) out.push({ id: ids[i], hedge: h })
    })
    if (done) break
  }
  return out
}

// ---- unités ----

export const AMOUNT_DECIMALS = 7
export const PRICE_DECIMALS = 14

export function toUnits(value: number | string, decimals = AMOUNT_DECIMALS): bigint {
  const [i, f = ''] = String(value).replace(',', '.').split('.')
  return BigInt(i || '0') * 10n ** BigInt(decimals) + BigInt((f + '0'.repeat(decimals)).slice(0, decimals) || '0')
}

export function fromUnits(v: bigint, decimals = AMOUNT_DECIMALS): number {
  return Number(v) / 10 ** decimals
}

export function fmtUsdc(v: bigint, digits = 0) {
  return fromUnits(v).toLocaleString('en-US', { minimumFractionDigits: digits, maximumFractionDigits: digits })
}

export function fmtRate(v: bigint) {
  return fromUnits(v, PRICE_DECIMALS).toLocaleString('en-US', { maximumFractionDigits: 6 })
}

// ---- erreurs (#[contracterror] : Vault 1xx, Paiement 2xx, Pricing 3xx) ----

const ERRORS: Record<number, string> = {
  100: 'Invalid amount.',
  101: 'The pool doesn’t have enough capacity for this hedge.',
  102: 'Internal error (release exceeds reserved).',
  103: 'Loss exceeds the pool’s capital.',
  104: 'The pool is depleted.',
  105: 'Deposit too small.',
  106: 'Not enough shares.',
  107: 'Withdrawal too small, or nothing free to withdraw right now.',
  108: 'Limit reached for this currency.',
  109: 'Internal error (hedge not registered in the pool).',
  110: 'Amount too large.',
  111: 'Dynamic must stay at least 20% of deposits: this deposit or withdrawal would break that.',
  112: 'You don’t hold that many Safe shares.',
  // Erreurs du contrat token USDC (SAC), hors spec du pool : #10 solde insuffisant, #13 trustline absente.
  10: 'Not enough USDC in your wallet.',
  13: 'Your wallet has no USDC trustline.',
  200: 'Hedge not found.',
  201: 'This hedge is already settled.',
  202: 'Maturity not reached yet.',
  203: 'Invalid invoice amount.',
  204: 'Invalid maturity.',
  205: 'Invalid demo time factor.',
  206: 'Maturity reached: settle this hedge instead.',
  300: 'No price available for this currency.',
  301: 'Invalid oracle price.',
  302: 'Oracle price is stale, try again in a moment.',
  303: 'Interest rate not set for this currency.',
  304: 'Maturity too far out.',
  305: 'Invalid invoice amount.',
  306: 'Currency not available.',
  307: 'Protection below this currency’s minimum.',
  308: 'Protection too high.',
  309: 'Rate too high.',
  310: 'Protection out of range.',
  311: 'Spread too high.',
  312: 'Settlement rate unavailable (oracle had no price at maturity).',
}

export function explainError(e: unknown): string {
  if (e instanceof ContractError) return translateDoc(e.doc)
  const msg = e instanceof Error ? e.message : String(e)
  // #10 = erreur du contrat token USDC (SAC) : solde insuffisant. Aucun code du pool ne vaut 10.
  if (/Error\(Contract, #10\)/.test(msg)) return 'Not enough USDC in your wallet.'
  const code = msg.match(/Error\(Contract, #(\d+)\)/)?.[1]
  if (code && ERRORS[Number(code)]) return ERRORS[Number(code)]
  if (/reject|declin|cancel/i.test(msg)) return 'Signature rejected in the wallet.'
  return msg.length > 160 ? msg.slice(0, 160) + '…' : msg
}

/** Doc des variants (contracts/pool/src/*.rs) → code d'erreur. Un fragment distinctif de chaque doc suffit. */
const DOC_CODES: [string, number][] = [
  ['Montant nul ou négatif', 100],
  ['`reserve` au-delà', 101],
  ['`release` au-delà', 102],
  ['`book_loss` rendrait', 103],
  ['plus de capital', 104],
  ['Dépôt trop petit', 105],
  ['parts senior que', 112],
  ['Retrait de plus de parts', 106],
  ['Retrait qui verserait 0', 107],
  ['limite de la devise', 108],
  ['`close_position`', 109],
  ['Débordement', 110],
  ['passerait sous', 111],
  ["id qui n'existe pas", 200],
  ['déjà réglée', 201],
  ["avant l'échéance", 202],
  ['`open` avec un notionnel', 203],
  ['`open` avec une durée', 204],
  ['`set_time_factor` à 0', 205],
  ['`close_early` à l', 206],
  ['aucun prix pour la devise', 300],
  ["Prix de l'oracle nul", 301],
  ['MAX_PRICE_AGE_SECS', 302],
  ['non posé par `set_rate`', 303],
  ['`set_rate` au-delà', 309],
  ['supérieure à un an', 304],
  ['`quote` avec un notionnel', 305],
  ['Devise non ouverte', 306],
  ['sous le minimum', 307],
  ['au-dessus de `MAX_CAP_BPS`', 308],
  ['`set_min_cap` à 0', 310],
  ['`set_spread`', 311],
  ['cours de fixing', 312],
]

function translateDoc(doc: string): string {
  const hit = DOC_CODES.find(([frag]) => doc.includes(frag))
  return hit ? ERRORS[hit[1]] : 'The contract rejected this operation.'
}
