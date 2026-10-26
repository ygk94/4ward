// [FRONT — Hugo] Barre du haut : navigation à gauche, wallet à droite.
import { useState } from 'react'
import { BRAND } from './config'
import { go, useRoute } from './router'
import { shortAddress, useWallet } from './wallet'

export function TopBar() {
  const route = useRoute()
  return (
    <header className="topbar">
      <nav className="topbar-left">
        <button className="brand" onClick={() => go('/')}>
          <span className="brand-mark" aria-hidden>◇</span>
          {BRAND}
        </button>
        {/* Sur l'accueil, pas de catégories : on y entre par le scroll (cf. Home.tsx). */}
        {route !== '/' && (
          <>
            <button className={`nav-pill ${route === '/vault' ? 'active' : ''}`} onClick={() => go('/vault')}>
              Vaults
            </button>
            <button className={`nav-pill ${route === '/positions' ? 'active' : ''}`} onClick={() => go('/positions')}>
              My positions
            </button>
            <button className={`nav-pill ${route === '/risk' ? 'active' : ''}`} onClick={() => go('/risk')}>
              Risk lab
            </button>
          </>
        )}
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
