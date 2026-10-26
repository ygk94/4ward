// [FRONT — Hugo] Chiffre qui défile de l'ancienne à la nouvelle valeur (requestAnimationFrame, sans librairie).
import { useEffect, useRef, useState } from 'react'

const reduced = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches

/** Valeur affichée qui rejoint `target` en `ms` (ease-out). Sans mouvement réduit : saut direct, à l'image suivante. */
export function useTween(target: number | undefined, ms = 400): number | undefined {
  const [shown, setShown] = useState(target)
  const from = useRef(target)

  useEffect(() => {
    if (target === undefined) return
    const a = from.current ?? target
    const dur = reduced() ? 0 : ms
    const start = performance.now()
    let raf = 0
    const step = (now: number) => {
      const k = dur === 0 ? 1 : Math.min(1, (now - start) / dur)
      const x = a + (target - a) * (1 - (1 - k) ** 3)
      from.current = x
      setShown(x)
      if (k < 1) raf = requestAnimationFrame(step)
    }
    raf = requestAnimationFrame(step)
    return () => cancelAnimationFrame(raf)
  }, [target, ms])

  return target === undefined ? undefined : (shown ?? target)
}
