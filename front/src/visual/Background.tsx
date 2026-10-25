// [FRONT — Hugo] Fond plein écran : carreaux crème qui ondulent (Acid Squares), avec des reflets irisés
// de la palette Glacier (bleu, cyan, glace, violet) qui glissent dessus, sans teinter tout le fond.
import AcidSquares from './AcidSquares'

const GLACIER = ['#2f6bff', '#6fd3ff', '#b9f3ff', '#8c7bff']

export function Background({ dim = false }: { dim?: boolean }) {
  return (
    <div className={`bg-fixed ${dim ? 'dim' : ''}`} aria-hidden>
      <AcidSquares color="#fbf8f1" palette={GLACIER} paletteSpeed={0.15} tint={0.28} lightMode gloss={1} brightness={1.1} speed={0.45} grain={0.04} />
    </div>
  )
}
