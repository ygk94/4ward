// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT — Hugo] Routage par hash (#/, #/about, #/vault, #/positions, #/risk) : pas de config serveur pour le déploiement statique.
import { useSyncExternalStore } from 'react'

export type Route = '/' | '/about' | '/vault' | '/positions' | '/risk'
const ROUTES: Route[] = ['/', '/about', '/vault', '/positions', '/risk']

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
