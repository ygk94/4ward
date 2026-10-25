// [VAULT — partagé] Mise en page générale et navigation entre les écrans.
import { HedgeScreen } from './payment/HedgeScreen'
import { CrashButton } from './pricing/CrashButton'
import { PoolPanel } from './vault/PoolPanel'

function App() {
  return (
    <main>
      <h1>&lt;NOM&gt;</h1>
      <HedgeScreen />
      <CrashButton />
      <PoolPanel />
    </main>
  )
}

export default App
