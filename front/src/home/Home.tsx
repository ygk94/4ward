// [FRONT — Hugo] Page d'accueil : titre, capital déposé ; le fond plein écran (Background) sert de décor.
// On entre en scrollant (molette, trackpad, ou swipe vers le haut sur mobile) : le bouton « Enter » se
// remplit et le vault de fond éclate au même rythme ; à 100 % on passe sur la page de présentation.
// Le remplissage reste en place à l'arrêt et ne recule que si on remonte. Un clic sur le bouton entre directement.
import { useEffect, useRef } from 'react'
import { fmtUsdc } from '../shared/contract'
import { go } from '../shared/router'
import { usePoolStats } from '../shared/usePool'
import { burst } from '../visual/burst'

/** Distance de scroll (px de molette) ou de swipe (px de doigt) pour remplir la barre. */
const WHEEL_DISTANCE = 900
const SWIPE_DISTANCE = 260

export function Home() {
  const { stats, demo } = usePoolStats()
  const fillRef = useRef<HTMLSpanElement>(null)
  const barRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    let progress = 0
    let touchY: number | null = null
    let done = false
    let raf = 0

    const push = (delta: number) => {
      if (done) return
      progress = Math.min(1, Math.max(0, progress + delta)) // remonter fait reculer
    }
    const onWheel = (e: WheelEvent) => {
      e.preventDefault()
      push(e.deltaY / WHEEL_DISTANCE)
    }
    const onTouchStart = (e: TouchEvent) => {
      touchY = e.touches[0].clientY
    }
    const onTouchMove = (e: TouchEvent) => {
      if (touchY === null) return
      const y = e.touches[0].clientY
      push((touchY - y) / SWIPE_DISTANCE) // doigt vers le haut = avancer
      touchY = y
    }
    const onKey = (e: KeyboardEvent) => {
      if (['ArrowDown', 'PageDown', ' ', 'Enter'].includes(e.key)) push(0.34)
      if (['ArrowUp', 'PageUp'].includes(e.key)) push(-0.34)
    }

    const frame = () => {
      fillRef.current?.style.setProperty('--p', String(progress))
      burst.home = progress // le vault de fond éclate au même rythme que le bouton se remplit
      barRef.current?.setAttribute('data-progress', String(Math.round(progress * 100)))
      if (progress >= 1 && !done) {
        done = true
        // petite pause sur le bouton plein avant de changer de page
        setTimeout(() => go('/about'), 180)
      }
      raf = requestAnimationFrame(frame)
    }
    raf = requestAnimationFrame(frame)

    window.addEventListener('wheel', onWheel, { passive: false })
    window.addEventListener('touchstart', onTouchStart, { passive: true })
    window.addEventListener('touchmove', onTouchMove, { passive: true })
    window.addEventListener('keydown', onKey)
    return () => {
      cancelAnimationFrame(raf)
      burst.home = 0
      window.removeEventListener('wheel', onWheel)
      window.removeEventListener('touchstart', onTouchStart)
      window.removeEventListener('touchmove', onTouchMove)
      window.removeEventListener('keydown', onKey)
    }
  }, [])

  return (
    <section className="home">
      <div className="home-copy">
        <p className="eyebrow">FX hedging for SMEs · Stellar</p>
        <h1 className="display">
          Lock today’s rate
          <br />
          <span className="accent">for tomorrow’s invoice.</span>
        </h1>
      </div>

      <div className="home-gauge">
        <p className="eyebrow">Total value locked {demo && <span className="chip">demo</span>}</p>
        <p className="gauge-num">
          {stats ? fmtUsdc(stats.totalAssets) : '—'}
          <span>USDC</span>
        </p>
        <button ref={barRef} className="enter-btn" onClick={() => go('/about')} aria-label="Enter (or scroll down)">
          <span ref={fillRef} className="enter-btn-fill" aria-hidden />
          <span className="enter-btn-label">
            Enter <span aria-hidden>→</span>
          </span>
        </button>
      </div>
    </section>
  )
}
