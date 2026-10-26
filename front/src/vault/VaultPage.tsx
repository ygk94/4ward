// [FRONT — Hugo] Page vault : infos du pool à gauche (2/3), choix du rôle et formulaire à droite (1/3).
import { useEffect, useState } from 'react'
import { HedgeScreen } from '../payment/HedgeScreen'
import { useActivity, usePoolStats } from '../shared/usePool'
import { clearPendingRole, pendingRole, type Role } from './entry'
import { LpForm } from './LpForm'
import { PoolPanel } from './PoolPanel'

export function VaultPage() {
  const { stats, demo, refresh } = usePoolStats()
  const { activity, refresh: refreshActivity } = useActivity()
  const [role, setRole] = useState<Role>(pendingRole)
  // Le rôle venu d'un lien ne vaut que pour cette ouverture ; « Vaults » dans la barre repart du choix.
  useEffect(() => {
    clearPendingRole()
  }, [])
  const onDone = () => {
    refresh()
    refreshActivity()
  }

  return (
    <div className="vault-page">
      <PoolPanel stats={stats} demo={demo} activity={activity} />

      <aside className="side-panel">
        {role === null ? (
          <div className="roles">
            <p className="eyebrow">What do you want to do?</p>
            <button className="role-card lp" onClick={() => setRole('lp')}>
              <span className="role-tag">Liquidity provider</span>
              <span className="role-title">Supply USDC</span>
              <span className="muted small">Be the counterparty to SMEs and earn the spreads.</span>
              <span className="role-go">→</span>
            </button>
            <button className="role-card sme" onClick={() => setRole('sme')}>
              <span className="role-tag">SME</span>
              <span className="role-title">Hedge an invoice</span>
              <span className="muted small">Lock today the price of a foreign-currency payment.</span>
              <span className="role-go">→</span>
            </button>
          </div>
        ) : (
          <div className="role-form">
            <button className="link back" onClick={() => setRole(null)}>
              ← Switch role{role === 'lp' && <span className="muted"> · Supply USDC</span>}
            </button>
            {role === 'sme' && <h2>Hedge an invoice</h2>}
            {role === 'lp' ? <LpForm onDone={onDone} /> : <HedgeScreen onDone={onDone} />}
          </div>
        )}
      </aside>
    </div>
  )
}
