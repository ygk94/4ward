// [FRONT] Logo 4ward : deux barres de même longueur, la seconde décalée vers l'avant
// (« même prix, plus tard »), barre du haut à l'encre, barre du bas en bleu.
export function LogoMark({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 272 154" aria-hidden>
      <path d="M4 63 L33 0 H219 L188 63 Z" fill="var(--ink)" />
      <path d="M52 153 L84 90 H271 L240 153 Z" fill="var(--blue)" />
    </svg>
  )
}
