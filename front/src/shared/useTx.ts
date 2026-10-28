// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT — Hugo] Exécution d'un appel au contrat : simulation, signature dans le wallet, envoi, attente.
import { useState } from 'react'
import type { contract } from '@stellar/stellar-sdk'
import { explainError, pool, unwrap, type PoolClient } from './contract'
import { connect, getAddress } from './wallet'

export type TxState =
  | { step: 'idle' }
  | { step: 'signing' }
  | { step: 'done'; result: unknown; hash?: string }
  | { step: 'error'; message: string }

export function useTx() {
  const [state, setState] = useState<TxState>({ step: 'idle' })

  async function run<T>(build: (p: PoolClient, address: string) => Promise<contract.AssembledTransaction<T>>) {
    try {
      const address = getAddress() ?? (await connect())
      setState({ step: 'signing' })
      const tx = await build(await pool(address), address)
      unwrap(tx.result) // la simulation a échoué : inutile d'ouvrir le wallet
      const sent = await tx.signAndSend()
      const result = unwrap(sent.result)
      setState({ step: 'done', result, hash: sent.sendTransactionResponse?.hash })
      return result
    } catch (e) {
      console.error('[tx]', e)
      setState({ step: 'error', message: explainError(e) })
    }
  }

  return { state, run, reset: () => setState({ step: 'idle' }) }
}

export function explorerTx(hash: string) {
  return `https://stellar.expert/explorer/testnet/tx/${hash}`
}
