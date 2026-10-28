// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT — Hugo] Bouclier « protected » : contour au dégradé Glacier, tracé en deux passes comme l'encre du vault,
// avec une coche qui se dessine. Version grande (About) et badge compact (Mes positions).
import { useId } from 'react'

const OUTLINE = 'M32 4 L56 12 V30 C56 45 46 55 32 60 C18 55 8 45 8 30 V12 Z'
const CHECK = 'M21 32 L29 40 L44 24'

export function Shield({ className, size }: { className?: string; size?: number }) {
  const id = useId()
  return (
    <svg className={`shield ${className ?? ''}`} viewBox="0 0 64 64" width={size} height={size} aria-hidden>
      <defs>
        <linearGradient id={`${id}-g`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#2f6bff" />
          <stop offset="0.55" stopColor="#6fd3ff" />
          <stop offset="1" stopColor="#8c7bff" />
        </linearGradient>
      </defs>
      <path d={OUTLINE} fill={`url(#${id}-g)`} opacity="0.1" />
      {/* seconde passe décalée, plus fine : effet trait à la main */}
      <path d={OUTLINE} fill="none" stroke="#8c7bff" strokeWidth="1" opacity="0.5" transform="translate(1.2 -0.8)" />
      <path className="shield-line" d={OUTLINE} fill="none" stroke={`url(#${id}-g)`} strokeWidth="2.6" strokeLinejoin="round" pathLength={1} />
      <path className="shield-check" d={CHECK} fill="none" stroke={`url(#${id}-g)`} strokeWidth="3.4" strokeLinecap="round" strokeLinejoin="round" pathLength={1} />
    </svg>
  )
}

export function ProtectedBadge({ capBps }: { capBps: number }) {
  return (
    <span className="protected" title={`Your loss is capped at ${capBps / 100}% of the invoice`}>
      <Shield size={16} />
      Protected ±{capBps / 100}%
    </span>
  )
}
