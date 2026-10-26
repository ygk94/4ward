// [FRONT — Hugo] Mise en page générale et navigation entre les écrans.
import { useEffect, useRef } from 'react'
import { About } from './home/About'
import { Home } from './home/Home'
import { PositionsPage } from './positions/PositionsPage'
import { RiskPage } from './risk/RiskPage'
import { useRoute } from './shared/router'
import { TopBar } from './shared/TopBar'
import { VaultPage } from './vault/VaultPage'
import { Background } from './visual/Background'
import { slide } from './visual/burst'

/** Ordre du menu : sert à donner un sens au glissement entre onglets. */
const ORDER = ['/vault', '/positions', '/risk']

function App() {
  const route = useRoute()
  const prevRoute = useRef(route)
  const dir = useRef(0)
  if (prevRoute.current !== route) {
    const [a, b] = [ORDER.indexOf(prevRoute.current), ORDER.indexOf(route)]
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    dir.current = a < 0 || b < 0 || reduced ? 0 : Math.sign(b - a)
    prevRoute.current = route
    slide.dir = dir.current
    slide.start = performance.now()
  }
  useEffect(() => {
    window.scrollTo(0, 0)
  }, [route])
  return (
    <>
      <Background dim={route !== '/'} rebuild={route === '/risk'} />
      <TopBar />
      <main className={`page page-${route.slice(1) || 'home'} ${dir.current > 0 ? 'slide-fwd' : dir.current < 0 ? 'slide-back' : ''}`} key={route}>
        {route === '/' && <Home />}
        {route === '/about' && <About />}
        {route === '/vault' && <VaultPage />}
        {route === '/positions' && <PositionsPage />}
        {route === '/risk' && <RiskPage />}
      </main>
    </>
  )
}

export default App
