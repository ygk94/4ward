// [FRONT — Hugo] Mise en page générale et navigation entre les écrans.
import { useEffect } from 'react'
import { About } from './home/About'
import { Home } from './home/Home'
import { PositionsPage } from './positions/PositionsPage'
import { useRoute } from './shared/router'
import { TopBar } from './shared/TopBar'
import { VaultPage } from './vault/VaultPage'
import { Background } from './visual/Background'

function App() {
  const route = useRoute()
  useEffect(() => {
    window.scrollTo(0, 0)
  }, [route])
  return (
    <>
      <Background dim={route !== '/'} />
      <TopBar />
      <main className={`page page-${route.slice(1) || 'home'}`} key={route}>
        {route === '/' && <Home />}
        {route === '/about' && <About />}
        {route === '/vault' && <VaultPage />}
        {route === '/positions' && <PositionsPage />}
      </main>
    </>
  )
}

export default App
