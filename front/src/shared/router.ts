// [FRONT — Hugo] Routage par hash (#/, #/about, #/vault, #/positions) : pas de config serveur pour le déploiement statique.
import { useSyncExternalStore } from 'react'

export type Route = '/' | '/about' | '/vault' | '/positions'
const ROUTES: Route[] = ['/', '/about', '/vault', '/positions']

function current(): Route {
  const r = window.location.hash.replace(/^#/, '') || '/'
  return (ROUTES as string[]).includes(r) ? (r as Route) : '/'
}

export function useRoute(): Route {
  return useSyncExternalStore(
    (l) => {
      window.addEventListener('hashchange', l)
      return () => window.removeEventListener('hashchange', l)
    },
    current,
  )
}

export function go(route: Route) {
  window.location.hash = route
  window.scrollTo(0, 0)
}
