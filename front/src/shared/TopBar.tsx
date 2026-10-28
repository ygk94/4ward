// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT — Hugo] Barre du haut : navigation à gauche, wallet à droite.
// L'onglet actif est marqué par une pastille bleue qui glisse d'un bouton à l'autre.
import { useState } from 'react'
import { BRAND } from './config'
import { LogoMark } from './Logo'
import { go, useRoute } from './router'
import { useSlidingPill } from './useSlidingPill'
import { shortAddress, useWallet } from './wallet'

const TABS = [
  { route: '/vault', label: 'Vaults' },
  { route: '/positions', label: 'My positions' },
  { route: '/risk', label: 'Risk lab' },
] as const

/** Onglets avec la pastille glissante (cf. useSlidingPill). */
function NavPills({ route }: { route: string }) {
  const { wrap, pill } = useSlidingPill(route)
  return (
    <div className="nav-pills has-pill" ref={wrap}>
      <span className="slide-pill nav-indicator" ref={pill} aria-hidden />
      {TABS.map((t) => (
        <button key={t.route} className={`nav-pill ${route === t.route ? 'active' : ''}`} onClick={() => go(t.route)}>
          {t.label}
        </button>
      ))}
    </div>
  )
}

export function TopBar() {
  const route = useRoute()
  return (
    <header className="topbar">
      <nav className="topbar-left">
        <button className="brand" onClick={() => go('/')}>
          <LogoMark className="brand-mark" />
          <span className="brand-word">
            {BRAND.slice(0, 1)}
            <i>{BRAND.slice(1)}</i>
          </span>
        </button>
        {/* Sur l'accueil, pas de catégories : on y entre par le scroll (cf. Home.tsx). */}
        {route !== '/' && <NavPills route={route} />}
      </nav>
      <WalletButton />
    </header>
  )
}

export function WalletButton() {
  const { address, connect, disconnect } = useWallet()
  const [open, setOpen] = useState(false)
  const [busy, setBusy] = useState(false)

  if (!address) {
    return (
      <button
        className="btn btn-wallet"
        disabled={busy}
        onClick={async () => {
          setBusy(true)
          try {
            await connect()
          } catch (e) {
            console.warn('[wallet] connexion annulée', e)
          } finally {
            setBusy(false)
          }
        }}
      >
        <span className="rainbow">{busy ? 'Connecting…' : 'Connect wallet'}</span>
      </button>
    )
  }

  return (
    <div className="wallet-menu">
      <button className="btn btn-wallet connected" onClick={() => setOpen((o) => !o)}>
        <span className="rainbow">{shortAddress(address)}</span>
      </button>
      {open && (
        <div className="wallet-dropdown" onMouseLeave={() => setOpen(false)}>
          <p className="mono small">{address}</p>
          <button className="link" onClick={() => navigator.clipboard?.writeText(address)}>
            Copy address
          </button>
          <button
            className="link danger"
            onClick={() => {
              setOpen(false)
              disconnect()
            }}
          >
            Disconnect
          </button>
        </div>
      )}
    </div>
  )
}
