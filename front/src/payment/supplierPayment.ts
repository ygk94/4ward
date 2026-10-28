// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT — Hugo] Paiement du fournisseur après le règlement : path payment strict receive USDC → devise.
// Opération Stellar classique (pas Soroban) : une transaction séparée de settle, signée par la PME dans son wallet.
// Strict receive : le fournisseur reçoit exactement le notionnel, la PME paie au plus sendMax.
import { Asset, BASE_FEE, contract, Horizon, Operation, TransactionBuilder } from '@stellar/stellar-sdk'
import { config } from '../shared/config'
import { PRICE_DECIMALS, type Hedge } from '../shared/contract'
import { connect, getAddress, signTransaction } from '../shared/wallet'

const HORIZON_URL = (import.meta.env.VITE_HORIZON_URL as string | undefined) ?? 'https://horizon-testnet.stellar.org'
const server = new Horizon.Server(HORIZON_URL)

/** Marge de glissement ajoutée au coût estimé (sendMax), en millièmes : 5 = 0,5 %. */
const SLIPPAGE_PER_MILLE = 5n

/** Devises payables sur le testnet : seules celles qui ont un actif classique et une offre DEX (setup-testnet.sh). */
export const PAYABLE = ['MXN']

const usdc = () => new Asset('USDC', config.issuer)
const currencyAsset = (code: string) => new Asset(code, config.issuer)

/** Montant en unités de 7 décimales → chaîne décimale attendue par Horizon ("1000000.0000000"). */
function toAmount(units: bigint): string {
  const s = units.toString().padStart(8, '0')
  return `${s.slice(0, -7)}.${s.slice(-7)}`
}

/** Chaîne décimale Horizon → unités de 7 décimales. */
function fromAmount(amount: string): bigint {
  const [int, frac = ''] = amount.split('.')
  return BigInt(int + frac.padEnd(7, '0').slice(0, 7))
}

export interface PaymentQuote {
  /** Coût estimé en USDC (7 décimales) pour livrer exactement le notionnel. */
  usdcCost: bigint
  path: Asset[]
}

/** Coût actuel sur le DEX pour livrer `notional` (7 décimales) de `currency` au fournisseur. */
export async function quoteSupplierPayment(currency: string, notional: bigint): Promise<PaymentQuote> {
  const res = await server.strictReceivePaths([usdc()], currencyAsset(currency), toAmount(notional)).call()
  if (res.records.length === 0) throw new Error(`Not enough ${currency} liquidity on the Stellar DEX right now.`)
  const best = res.records.reduce((a, b) => (Number(b.source_amount) < Number(a.source_amount) ? b : a))
  return {
    usdcCost: fromAmount(best.source_amount),
    path: best.path.map((p) => (p.asset_type === 'native' ? Asset.native() : new Asset(p.asset_code, p.asset_issuer))),
  }
}

/**
 * Signe et envoie le path payment. Retourne le hash et le montant USDC réellement payé (lu sur Horizon).
 * Le coût est re-coté juste avant la signature : le DEX a pu bouger depuis l'aperçu.
 */
export async function paySupplier(
  currency: string,
  notional: bigint,
  supplier: string,
): Promise<{ hash: string; usdcPaid: bigint }> {
  const address = getAddress() ?? (await connect())
  const quote = await quoteSupplierPayment(currency, notional)
  const sendMax = (quote.usdcCost * (1000n + SLIPPAGE_PER_MILLE) + 999n) / 1000n

  const account = await server.loadAccount(address)
  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: config.networkPassphrase })
    .addOperation(
      Operation.pathPaymentStrictReceive({
        sendAsset: usdc(),
        sendMax: toAmount(sendMax),
        destination: supplier,
        destAsset: currencyAsset(currency),
        destAmount: toAmount(notional),
        path: quote.path,
      }),
    )
    .setTimeout(180)
    .build()

  try {
    const { signedTxXdr } = await signTransaction(tx.toXDR(), { address })
    const sent = await server.submitTransaction(TransactionBuilder.fromXDR(signedTxXdr, config.networkPassphrase))
    const ops = await server.operations().forTransaction(sent.hash).call()
    const op = ops.records[0] as unknown as { source_amount?: string }
    return { hash: sent.hash, usdcPaid: op?.source_amount ? fromAmount(op.source_amount) : quote.usdcCost }
  } catch (e) {
    throw new Error(explainHorizonError(e))
  }
}

/** Codes de résultat Horizon → message lisible. */
function explainHorizonError(e: unknown): string {
  const codes = (e as { response?: { data?: { extras?: { result_codes?: { transaction?: string; operations?: string[] } } } } })
    ?.response?.data?.extras?.result_codes
  const op = codes?.operations?.[0]
  switch (op) {
    case 'op_too_few_offers':
      return 'Not enough liquidity on the Stellar DEX to deliver this amount.'
    case 'op_over_source_max':
      return 'The DEX price moved while signing. Try again.'
    case 'op_underfunded':
      return 'Not enough USDC in your wallet.'
    case 'op_no_trust':
    case 'op_src_no_trust':
      return 'A trustline is missing (your USDC or the supplier’s currency).'
    case 'op_no_destination':
      return 'The supplier account does not exist.'
  }
  if (codes?.transaction === 'tx_bad_seq') return 'Your account sequence changed. Try again.'
  if (codes?.transaction === 'tx_insufficient_balance') return 'Not enough XLM to pay the network fee.'
  const msg =
    e instanceof Error ? e.message : e && typeof e === 'object' && 'message' in e ? String((e as { message: unknown }).message) : String(e)
  if (/reject|declin|cancel|closed the modal/i.test(msg)) return 'Signature rejected in the wallet.'
  return codes ? `Payment failed (${op ?? codes.transaction}).` : msg
}

// Mémoire locale du parcours (versement du règlement, paiement fait), pour afficher le coût total après rechargement.
/** Clés par pool : les ids de couverture repartent de 0 à chaque redéploiement. */
export const key = (kind: string, id: bigint) => `fx.${kind}.${config.poolId}.${id}`

export function rememberPayout(id: bigint, payout: bigint) {
  try {
    localStorage.setItem(key('payout', id), payout.toString())
  } catch {
    // stockage indisponible : le coût total ne sera affiché que dans cette session
  }
}

export function recallPayout(id: bigint): bigint | null {
  try {
    const v = localStorage.getItem(key('payout', id))
    return v === null ? null : BigInt(v)
  } catch {
    return null
  }
}

/**
 * P&L du règlement recalculé sans dépendre du navigateur : cours de l'oracle à l'échéance (le même fixing que
 * settle, pricing::fixing) puis la formule de hedge::pnl (arrondi vers le bas, borné à ± la marge).
 * Lit le mock-oracle du testnet (config.mockOracleId) ; null si l'oracle n'a plus ce prix dans son historique.
 */
export async function settledPnl(hedge: Hedge): Promise<bigint | null> {
  try {
    const oracle = (await contract.Client.from({
      contractId: config.mockOracleId,
      rpcUrl: config.rpcUrl,
      networkPassphrase: config.networkPassphrase,
    })) as unknown as {
      price: (a: { asset: { tag: 'Other'; values: [string] }; timestamp: bigint }) => Promise<{ result: { price: bigint } | undefined }>
    }
    const data = (await oracle.price({ asset: { tag: 'Other', values: [hedge.currency] }, timestamp: hedge.maturity })).result
    if (!data) return null
    const diff = hedge.side.tag === 'BuyCurrency' ? data.price - hedge.locked_rate : hedge.locked_rate - data.price
    const scale = 10n ** BigInt(PRICE_DECIMALS)
    const prod = hedge.notional * diff
    const raw = prod >= 0n ? prod / scale : -((-prod + scale - 1n) / scale) // div_euclid : arrondi vers le bas
    return raw > hedge.margin ? hedge.margin : raw < -hedge.margin ? -hedge.margin : raw
  } catch (e) {
    console.warn('[settledPnl]', e)
    return null
  }
}

export interface PaidRecord {
  hash: string
  usdcPaid: bigint
}

export function rememberPaid(id: bigint, rec: PaidRecord) {
  try {
    localStorage.setItem(key('paid', id), JSON.stringify({ hash: rec.hash, usdcPaid: rec.usdcPaid.toString() }))
  } catch {
    // idem
  }
}

export function recallPaid(id: bigint): PaidRecord | null {
  try {
    const v = localStorage.getItem(key('paid', id))
    if (!v) return null
    const o = JSON.parse(v) as { hash: string; usdcPaid: string }
    return { hash: o.hash, usdcPaid: BigInt(o.usdcPaid) }
  } catch {
    return null
  }
}
