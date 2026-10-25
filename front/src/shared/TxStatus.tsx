// [FRONT — Hugo] Retour visuel d'une transaction (signature, succès, erreur).
import { explorerTx, type TxState } from './useTx'

export function TxStatus({ state, success }: { state: TxState; success?: (result: unknown) => string }) {
  if (state.step === 'idle') return null
  if (state.step === 'signing')
    return <p className="tx tx-wait">Confirm the transaction in your wallet…</p>
  if (state.step === 'error') return <p className="tx tx-err">{state.message}</p>
  return (
    <p className="tx tx-ok">
      {success ? success(state.result) : 'Transaction confirmed.'}{' '}
      {state.hash && (
        <a href={explorerTx(state.hash)} target="_blank" rel="noreferrer">
          View on explorer ↗
        </a>
      )}
    </p>
  )
}
