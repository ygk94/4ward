// [FRONT — Hugo] Page de présentation, à défilement : ce qu'on propose aux PME et aux fournisseurs de liquidité.
import Lenis from 'lenis'
import { useEffect, useRef } from 'react'
import { fmtUsdc } from '../shared/contract'
import { BRAND, TRADABLE } from '../shared/config'
import { usePoolStats } from '../shared/usePool'
import { openVault } from '../vault/entry'
import { Shield } from '../visual/Shield'

const CURRENCIES = ['MXN', 'BRL', 'ARS', 'EUR', 'COP', 'CLP', 'PEN', 'PHP', 'INR', 'NGN', 'KES', 'ZAR', 'TRY', 'GBP', 'JPY']

/** Ajoute la classe `in` quand l'élément entre à l'écran. */
function useReveal() {
  const root = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const io = new IntersectionObserver(
      (entries) => entries.forEach((e) => e.isIntersecting && e.target.classList.add('in')),
      { threshold: 0.15 },
    )
    root.current?.querySelectorAll('.reveal').forEach((el) => io.observe(el))
    return () => io.disconnect()
  }, [])
  return root
}

function useSmoothScroll() {
  useEffect(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    const lenis = new Lenis({ lerp: 0.09 })
    let raf = 0
    const loop = (time: number) => {
      lenis.raf(time)
      raf = requestAnimationFrame(loop)
    }
    raf = requestAnimationFrame(loop)
    return () => {
      cancelAnimationFrame(raf)
      lenis.destroy()
    }
  }, [])
}

export function About() {
  const root = useReveal()
  useSmoothScroll()
  const { stats } = usePoolStats()

  return (
    <div className="about" ref={root}>
      {/* 00 — ouverture */}
      <section className="about-hero">
        <p className="eyebrow reveal">{BRAND} — FX hedging for SMEs</p>
        <h1 className="mega">
          <span className="reveal">Pay later.</span>
          <span className="reveal indent">At today’s</span>
          <span className="reveal indent accent">price.</span>
        </h1>
        <p className="lede reveal">
          Lock the exchange rate of a future payment now, in USDC. On the due date, your supplier is paid in their own
          currency, at the promised rate. No bank, no credit line.
        </p>
        <p className="scroll-hint reveal">Scroll ↓</p>
      </section>

      <div className="marquee" aria-hidden>
        <div className="marquee-track">
          {[...CURRENCIES, ...CURRENCIES].map((c, i) => (
            <span key={i}>{c}</span>
          ))}
        </div>
      </div>
      <p className="marquee-legend">
        Tradable today: {TRADABLE.join(', ')} · the others are on the roadmap
      </p>

      {/* 01 — le problème */}
      <section className="about-block">
        <p className="section-num reveal">01 — The problem</p>
        <h2 className="big reveal">
          Between the invoice and the payment, the <span className="accent-pink">rate moves</span>.
        </h2>
        <p className="lede reveal">
          Large companies call their bank to lock it. SMEs can’t: no credit line, no FX desk. The swing eats their margin.
        </p>
      </section>

      {/* 02 — PME */}
      <section className="about-split">
        <div className="split-head">
          <p className="section-num reveal">02 — For SMEs</p>
          <Shield className="shield-xl reveal" />
          <h2 className="big reveal">Lock it. Cap it. Forget it.</h2>
          <p className="lede reveal">A locked price, with the protection you choose.</p>
        </div>
        <div className="cards">
          <article className="card reveal">
            <p className="card-k">~0.6%</p>
            <p className="card-l">of the invoice, all-in, for 90 days</p>
            <p className="muted small">No credit line needed</p>
          </article>
          <article className="card reveal">
            <p className="card-k">5–20%</p>
            <p className="card-l">margin of your choice = your protection</p>
            <p className="muted small">No margin calls, no liquidation</p>
          </article>
          <article className="card reveal">
            <p className="card-k">5 s</p>
            <p className="card-l">to pay the supplier in their currency</p>
            <p className="muted small">Stellar path payment</p>
          </article>
        </div>
        <button className="btn btn-ghost btn-enter reveal" onClick={() => openVault('sme')}>
          Hedge an invoice <span aria-hidden>→</span>
        </button>
      </section>

      {/* 03 — LP */}
      <section className="about-split alt">
        <div className="split-head">
          <p className="section-num reveal">03 — For liquidity providers</p>
          <h2 className="big reveal">Be the shield. Keep the fees.</h2>
          <p className="lede reveal">Your USDC backs every hedge, and earns a fee on each one.</p>
        </div>
        <div className="cards">
          <article className="card reveal">
            <p className="card-k">0.5%</p>
            <p className="card-l">spread on every hedge opened</p>
            <p className="muted small">Collected by the pool, shared through the share price</p>
          </article>
          <article className="card reveal">
            <p className="card-k">~4%</p>
            <p className="card-l">target on idle capital in tokenized US Treasuries</p>
            <p className="muted small">Roadmap, not live on testnet</p>
          </article>
          <article className="card reveal">
            <p className="card-k">0</p>
            <p className="card-l">credit risk: every max loss is deposited or reserved upfront</p>
            <p className="muted small">Reserved capital ≤ Dynamic tranche capital, checked by the contract at each new hedge</p>
          </article>
        </div>
        <div className="lp-cta reveal">
          <button className="btn btn-primary btn-enter" onClick={() => openVault('lp')}>
            Supply USDC <span aria-hidden>→</span>
          </button>
          <p className="muted small">
            {stats ? `${fmtUsdc(stats.totalAssets)} USDC already in the vault` : 'Deposit and withdraw anytime'}
          </p>
        </div>
      </section>

      {/* 04 — comment */}
      <section className="about-block">
        <p className="section-num reveal">04 — How it works</p>
        <ol className="steps">
          <li className="reveal">
            <span>01</span>
            <div>
              <h3>Enter the invoice</h3>
              <p className="muted">Amount, currency, due date, supplier address.</p>
            </div>
          </li>
          <li className="reveal">
            <span>02</span>
            <div>
              <h3>Get a locked price</h3>
              <p className="muted">Today’s rate, adjusted for 90 days: “this is what you’ll pay”, as long as the currency stays within your protection.</p>
            </div>
          </li>
          <li className="reveal">
            <span>03</span>
            <div>
              <h3>Sign in your wallet</h3>
              <p className="muted">The margin is deposited into the contract. Nothing more can ever be asked of you.</p>
            </div>
          </li>
          <li className="reveal">
            <span>04</span>
            <div>
              <h3>On the due date, it all settles</h3>
              <p className="muted">The pool pays or takes the difference, and the supplier receives exactly their pesos.</p>
            </div>
          </li>
        </ol>
      </section>

      {/* 05 — chiffres + accès */}
      <section className="about-cta">
        <p className="section-num reveal">05 — The vault, live</p>
        <p className="mega-num reveal">{stats ? fmtUsdc(stats.totalAssets) : '—'}</p>
        <p className="lede reveal">USDC deposited on testnet, ready to hedge invoices.</p>
        <button className="btn btn-primary btn-enter reveal" onClick={() => openVault(null)}>
          Go to vaults <span aria-hidden>→</span>
        </button>
      </section>

      <footer className="about-foot">
        <span>{BRAND}</span>
        <span className="muted">HackMeridian · Lisbon · 2026</span>
        <span className="muted">Reflector · Soroban · Freighter</span>
      </footer>
    </div>
  )
}
