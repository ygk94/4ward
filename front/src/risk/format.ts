// [FRONT — Hugo] Format des pourcentages du Risk lab (signe typographique « − », chiffres en-US).
export const pct = (x: number, digits = 1) =>
  `${x > 0 ? '+' : x < 0 ? '−' : ''}${Math.abs(x * 100).toLocaleString('en-US', { maximumFractionDigits: digits, minimumFractionDigits: digits })}%`
