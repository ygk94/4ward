// [FRONT — Hugo] Fond plein écran. Sur le crème du body, le vault filaire (PolyVault, en Canvas 2D), qui
// éclate quand on défile sur l'accueil ; sur les autres pages, ses barres deviennent un graphique (cf. burst.ts).
// Les carreaux Acid Squares ne sont plus affichés.
import { closed, homeBurst, opened, riskProgress } from './burst'
import { PolyVault } from './PolyVault'

export function Background({ dim = false, rebuild = false }: { dim?: boolean; rebuild?: boolean }) {
  return (
    <div className={`bg-vault ${dim ? 'dim' : ''}`} aria-hidden>
      <PolyVault
        className="bg-vault-canvas"
        burst={dim ? opened : homeBurst}
        chart={dim ? opened : closed}
        rebuild={rebuild ? riskProgress : undefined}
      />
    </div>
  )
}
