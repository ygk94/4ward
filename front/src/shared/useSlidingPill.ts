// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT] Pastille qui glisse d'un bouton à l'autre dans un groupe (onglets, choix du marché…).
// Le conteneur porte la classe `has-pill` et contient un <span className="slide-pill" /> ; la pastille
// se cale sur le bouton enfant `.active` ou `.on` (position et taille), avec une transition CSS.
import { useLayoutEffect, useRef } from 'react'

export function useSlidingPill<T extends HTMLElement = HTMLDivElement>(dep: unknown) {
  const wrap = useRef<T>(null)
  const pill = useRef<HTMLSpanElement>(null)

  useLayoutEffect(() => {
    const place = () => {
      const p = pill.current
      const el = wrap.current?.querySelector<HTMLElement>(':scope > .active, :scope > .on')
      if (!p) return
      if (!el) {
        p.style.opacity = '0'
        return
      }
      // première apparition : on pose la pastille sans la faire glisser depuis le coin
      if (p.style.opacity !== '1') p.style.transition = 'none'
      p.style.width = `${el.offsetWidth}px`
      p.style.height = `${el.offsetHeight}px`
      p.style.transform = `translate(${el.offsetLeft}px, ${el.offsetTop}px)`
      p.style.opacity = '1'
      void p.offsetWidth
      p.style.transition = ''
    }
    place()
    // recale si le groupe ou un bouton change de taille (police chargée, fenêtre redimensionnée)
    const ro = new ResizeObserver(place)
    if (wrap.current) {
      ro.observe(wrap.current)
      wrap.current.querySelectorAll(':scope > button').forEach((b) => ro.observe(b))
    }
    return () => ro.disconnect()
  }, [dep])

  return { wrap, pill }
}
