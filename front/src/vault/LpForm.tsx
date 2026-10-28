// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT — Hugo] Formulaire fournisseur de liquidité : deux façons de déposer, Safe (tranche senior) et Dynamic (junior).
// Barre de répartition du pool au-dessus ; tous les appels passent par ./poolApi.
import { useCallback, useEffect, useState } from 'react'
import { fmtUsdc, toUnits } from '../shared/contract'
import { TxStatus } from '../shared/TxStatus'
import { useTx } from '../shared/useTx'
import { useWallet } from '../shared/wallet'
import { deposit, MIN_DYNAMIC_SHARE, readMine, readSplit, SAFE_APR, withdraw, type Kind, type Mine, type Split } from './poolApi'

export function LpForm({ onDone }: { onDone: () => void }) {
  const { address } = useWallet()
  const [split, setSplit] = useState<Split | null>(null)
  const [mine, setMine] = useState<Mine | null>(null)
  const [kind, setKind] = useState<Kind>('dynamic')

  const load = useCallback(async () => {
    try {
      setSplit(await readSplit())
      setMine(address ? await readMine(address) : null)
    } catch (e) {
      console.warn('[lp] lecture impossible', e)
    }
  }, [address])

  useEffect(() => {
    load()
  }, [load])

  const done = () => {
    load()
    onDone()
  }

  return (
    <div className="form">
      <SplitBar split={split} />
      <div className="tranche-pick" role="tablist">
        {(['safe', 'dynamic'] as const).map((k) => (
          <button
            key={k}
            role="tab"
            aria-selected={kind === k}
            className={`pick-${k} ${kind === k ? 'on' : ''}`}
            onClick={() => setKind(k)}
            title={COPY[k].finance}
          >
            <b>
              {COPY[k].name} <i className="info">ⓘ</i>
            </b>
            <span>{k === 'safe' ? `${SAFE_APR}% target rate` : `variable · ${split ? fmtUsdc(split.sharePrice, 4) : '—'}/share`}</span>
          </button>
        ))}
      </div>
      <TrancheCard key={kind} kind={kind} split={split} mine={mine} onDone={done} />
    </div>
  )
}

function SplitBar({ split }: { split: Split | null }) {
  if (!split) return <div className="split skeleton" />
  const total = split.total > 0n ? split.total : 1n
  const pct = (v: bigint) => Number((v * 10_000n) / total) / 100
  const safe = pct(split.safe)
  const free = pct(split.dynamic - split.locked)
  const locked = pct(split.locked)
  return (
    <div className="split">
      <div className="split-top">
        <span className="small">Pool split</span>
        {split.capacity >= 0n ? <span className="badge ok">Solvent</span> : <span className="badge ko">Under-capitalized</span>}
      </div>
      <div className="split-bar" role="img" aria-label={`Safe ${safe}%, Dynamic free ${free}%, locked ${locked}%`}>
        <div className="seg-safe" style={{ width: `${safe}%` }} />
        <div className="seg-dyn" style={{ width: `${free}%` }} />
        <div className="seg-locked" style={{ width: `${locked}%` }} />
      </div>
      <div className="split-legend">
        <span>
          <i className="sw seg-safe" /> Safe {fmtUsdc(split.safe)}
        </span>
        <span>
          <i className="sw seg-dyn" /> Dynamic {fmtUsdc(split.dynamic)}
        </span>
        <span>
          <i className="sw seg-locked" /> locked by open hedges {fmtUsdc(split.locked)}
        </span>
      </div>
    </div>
  )
}

const COPY: Record<Kind, { name: string; line: string; rule: string; finance: string }> = {
  safe: {
    name: 'Safe',
    line: `Target rate of ${SAFE_APR}% a year, paid first. Not guaranteed: on testnet it is paid by Dynamic, not by T-bills. Losses hit Dynamic first.`,
    rule: `Paid by Dynamic until idle capital is invested (Etherfuse, roadmap). Refused if Dynamic would fall under ${MIN_DYNAMIC_SHARE}% of deposits.`,
    finance: 'In finance, this is called a senior tranche.',
  },
  dynamic: {
    name: 'Dynamic',
    line: 'Takes the risk, earns the rest: SME spreads and favourable FX moves, minus Safe’s target rate. No fixed return.',
    rule: `New hedges only use Dynamic capital not locked by open hedges. Must stay at least ${MIN_DYNAMIC_SHARE}% of deposits.`,
    finance: 'In finance, this is called a junior tranche.',
  },
}

function TrancheCard({ kind, split, mine, onDone }: { kind: Kind; split: Split | null; mine: Mine | null; onDone: () => void }) {
  const [amount, setAmount] = useState('')
  const [last, setLast] = useState<'deposit' | 'withdraw'>('deposit')
  const { state, run, reset } = useTx()
  const available = kind === 'dynamic' || !!split?.tranches
  const position = kind === 'dynamic' ? mine?.dynamic.balance : mine?.safe?.balance
  // même saisie que l'écran de couverture : virgule des milliers ignorée, point décimal, rien d'autre (pas de « 1e3 »)
  const clean = amount.replace(/[\s_,]/g, '')
  const value = /^\d+(\.\d{0,7})?$/.test(clean) ? Number(clean) : 0
  const busy = state.step === 'signing'
  const copy = COPY[kind]

  const act = async (action: 'deposit' | 'withdraw' | 'withdraw-all') => {
    if (!split) return
    reset()
    setLast(action === 'deposit' ? 'deposit' : 'withdraw')
    const build =
      action === 'deposit'
        ? deposit(kind, toUnits(clean))
        : withdraw(kind, action === 'withdraw-all' ? 'all' : toUnits(clean), split, mine ?? { dynamic: { balance: 0n, shares: 0n }, safe: null })
    const res = await run(build)
    if (res !== undefined) {
      setAmount('')
      onDone()
    }
  }

  return (
    <article className={`tranche tranche-${kind} ${available ? '' : 'disabled'}`}>
      <p className="small">{copy.line}</p>
      <p className="muted tiny">{copy.rule}</p>

      {!available ? (
        <p className="soon">Coming soon</p>
      ) : (
        <>
          <p className="small">
            Your position: <b>{position !== undefined ? `${fmtUsdc(position, 2)} USDC` : '—'}</b>
          </p>
          <div className="input-wrap">
            <input inputMode="decimal" placeholder="0" value={amount} onChange={(e) => setAmount(e.target.value)} />
            <em>USDC</em>
          </div>
          <div className="tranche-actions">
            <button className="btn btn-primary" disabled={busy || value <= 0} onClick={() => act('deposit')}>
              Deposit
            </button>
            <button className="btn btn-ghost" disabled={busy || value <= 0 || !position} onClick={() => act('withdraw')}>
              Withdraw
            </button>
            <button className="link" title="Withdraw your whole position" disabled={busy || !position} onClick={() => act('withdraw-all')}>
              Withdraw all
            </button>
          </div>
          <TxStatus
            state={state}
            success={(r) =>
              last === 'deposit'
                ? `Deposit confirmed: you now hold ${fmtUsdc(r as bigint, 2)} ${copy.name} shares.`
                : `Withdrawal confirmed: ${fmtUsdc(r as bigint, 2)} USDC sent.`
            }
          />
        </>
      )}
    </article>
  )
}
