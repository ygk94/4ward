# Mémo front — partie Pricing (pour Hugo)

De : Lancelot (Pricing). Pour : `QuoteCard`, `CrashButton` et les messages d'erreur.
Tout le code ci-dessous est **un seul fichier prêt à copier** dans `front/src/pricing/pricing.ts` (§ 7). Il n'utilise que `@stellar/stellar-sdk` (17.4.0, déjà dans `front/package.json`) et `config` de `shared/config.ts` ; pas de client généré, pas de `any`, montants en `bigint`.

**Vérifié contre le testnet le 25/10/2026 à 21:59 UTC** (script Node hors dépôt, sortie en § 8) :

| | ID |
|---|---|
| pool | `CAXH7TE6BA3LI237FIBK5AWDQ335JKY3KKAGQYRVAVQOIOK4VL64YPXC` (déploiement actuel ; référence : `.env.testnet`) |
| mock-oracle | `CBBNIAYYP6GBS25HE3DJ46X5VINNV4R2GHR4XBROARAW4EYV6APYEPKK` (idem) |
| Reflector FX (testnet) | `CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W` |

Les IDs vérifiés le 25/10 à 21:59 étaient ceux d'un déploiement antérieur ; à chaque redéploiement, **seuls les IDs de `.env.testnet` / `front/.env.local` changent** : les noms de fonctions, l'encodage et les décimales restent les mêmes.

---

## 1. Lire le cours (sans wallet)

`lastprice(Asset::Other("MXN"))`, en **simulation** (aucune signature, aucun frais), sur deux contrats qui ont la même interface :

- `config.reflectorFxId` → afficher **« Cours réel (Reflector) : 1 MXN = 0,0533 $ »**
- `config.mockOracleId` → afficher **« Cours de démo : 1 MXN = 0,0550 $ »**, et l'écart quand ils divergent (« +3,28 % »)

Encodage de l'argument : `Asset::Other(Symbol)` = `scvVec([scvSymbol('Other'), scvSymbol('MXN')])` (`assetOther`).
Retour : `Option<PriceData { price: i128, timestamp: u64 }>` → `null` si pas de prix, sinon deux `bigint` : `price` en **14 décimales** (USD par unité de devise), `timestamp` en secondes Unix. `fmtPrice14(price)` → « 0,0550 ».

**Âge du prix.** Le contrat refuse un prix de plus de **900 s** (code 302). Si `priceAgeSecs(demo) > 900n` : afficher « Le cours de démo a plus de 15 min » et un bouton **« Rafraîchir le cours »** (§ 5). Le mock n'est horodaté qu'à chaque `set_price` ; Reflector se met à jour toutes les 5 min.

## 2. Devis : `pool.quote`

Simulation de `quote(currency, side, notional, duration_secs, cap_bps) -> Quote` (`quote()` en § 7).

| Argument | Type Rust | Encodage | Exemple |
|---|---|---|---|
| `currency` | `Symbol` | `scvSymbol('MXN')` | `'MXN'` |
| `side` | `Side` (enum sans données) | `scvVec([scvSymbol('BuyCurrency')])` | `'BuyCurrency'` (importateur) / `'SellCurrency'` (exportateur) |
| `notional` | `i128`, 7 déc. | `nativeToScVal(n, { type: 'i128' })` | `10_000_000_000_000n` = 1 000 000 MXN (`parseAmount7('1 000 000')`) |
| `duration_secs` | `u64` | `nativeToScVal(BigInt(d), { type: 'u64' })` | `7_776_000` = 90 j |
| `cap_bps` | `u32` | `nativeToScVal(c, { type: 'u32' })` | `1000` = ±10 % |

| Champ de `Quote` | Décimales | Sens | Valeur de référence (1 000 000 MXN, spot 0,055, 90 j, ±10 %) |
|---|---|---|---|
| `spot` | 14 | cours du jour | `5500000000000` → 0,0550 |
| `forward_rate` | 14 | **prix figé** par MXN | `5501281368668` → 0,055013 |
| `usdc_cost` | 7 | ce que la PME paiera | `550128136866` → **55 012,81 USDC** |
| `margin` | 7 | dépôt de garantie | `55012813687` → **5 501,28 USDC** |

**Durée : passer la durée ANNONCÉE** (90 j = `7_776_000`), jamais la durée réelle de la démo. Sur `feat/payment` (Clément, option b) : `open_hedge` calcule le prix sur `duration_secs` et fixe l'échéance réelle à `maintenant + max(duration_secs / time_factor, 1)`. Avec `set_time_factor(25920)`, 90 j → **300 s réelles**. Le compte à rebours se fait sur `get_hedge(id).maturity` (échéance réelle, secondes Unix), pas sur la durée saisie.

**Durée ≥ 1 s** : `quote` accepte 0 mais `open_hedge` le refuse (204). Imposer au moins 1 s dans le formulaire.

Rafraîchir le devis quand les champs changent (debounce ~300 ms). `onQuote(q)` remonte `q.margin` à l'écran Paiement : c'est le montant que Lucía dépose (identique au centime à ce que `open_hedge` prélève : même fonction `margin_for` côté contrat).

## 3. Textes à l'écran (zéro jargon)

Pour la référence ci-dessus (arrondir à l'USDC dans les gros titres) :

- **« Prix figé : vous paierez 55 013 USDC, quoi qu'il arrive »** — `fmtAmount7(q.usdc_cost, 0)`
- **« Protection ±10 % »** — `capBps / 100`
- **« Dépôt de garantie : 5 501 USDC (restitué à l'échéance, ± l'écart) »** — `fmtAmount7(q.margin, 0)`
- Petit texte : « Prix figé = cours du jour (0,0550 $) ajusté de l'écart de taux entre le peso (5,55 %) et le dollar (3,6 %) »
- **Coût de la protection** (`protectionCost(q, durée)`) : « ≈ 323 USDC (0,6 % de la facture) : 274 de rémunération du pool + 49 de rendement perdu sur le dépôt ». Les constantes `SPREAD_BPS = 50` et `USD_RATE_BPS = 360` reprennent `scripts/setup-testnet.sh` et `scripts/rates.snapshot` (le contrat ne les expose pas).
- Après le crash +12 % (`afterMove(q, notional, side, nouveauSpot)`, mêmes formules que `settle`) :
  - « Sans protection : **61 600 USDC** » (`withoutProtection`)
  - « Avec <NOM> : **56 098,72 USDC** » (`withProtection`) ; protection versée 5 501,28 (plafonnée au dépôt)

Si le mock est aligné sur Reflector avant la vidéo, tout se recalcule tout seul depuis `quote` ; à 0,05325 : prix figé 53 262,41, avec protection 54 313,76 (`sim/out/chiffres-demo.md`).

## 4. Erreurs

Une simulation qui échoue renvoie un texte qui contient `Error(Contract, #302)`. `ContractCallError` en extrait `code` (`302`), `errorMessage(e, devise)` le traduit. Les textes des `panic!` sont effacés dans le contrat compilé : **seul le code compte**.

| Code | Quand | Message |
|---|---|---|
| 300 | pas de cours pour la devise | Cours indisponible pour cette devise |
| 301 | cours ≤ 0 | Cours invalide, réessayez plus tard |
| 302 | cours de plus de 15 min | Cours trop ancien : rafraîchissez le cours |
| 303 | taux d'intérêt non posé | Devise pas encore configurée |
| 304 | durée > 1 an | Échéance maximale : 1 an |
| 305 | montant ≤ 0 | Montant invalide |
| 306 | devise non ouverte (liste blanche) | Devise non disponible |
| 307 | protection sous le minimum | Protection minimale pour cette devise : 5 % (MXN 5, BRL 7, ARS 15, EUR 3) |
| 308 | protection > 50 % | Protection maximale : 50 % |
| 309-311 | admin (`set_rate`, `set_min_cap`, `set_spread`) | — |
| 312 | `settle` : pas de cours à l'échéance | Cours de règlement indisponible |
| 10 | token USDC (SAC) : solde insuffisant au transfert de la marge (`open_hedge`) ou au dépôt LP | Solde USDC insuffisant |
| 111 | Vault (tranches) : dépôt senior / retrait junior qui ferait passer le junior sous son minimum | Dépôt impossible pour l'instant : la part junior du pool est trop faible |
| 112 | Vault (tranches) : retrait de plus de parts senior que détenues | Vous n'avez pas assez de parts senior |

Codes 100-112 (Vault), 200-205 (Paiement ; 205 = `set_time_factor` à 0, admin) et #10 (token USDC : solde insuffisant) : table complète dans `docs/payment-cli.md` § « Erreurs à traduire » ; les ajouter au même `ERROR_MESSAGES`.
Contrôle de l'ordre : `quote` vérifie montant et plafond **avant** de lire l'oracle, donc 305 / 306 / 307 / 308 sortent même si le cours est périmé (vérifié en § 8).

## 5. `CrashButton`

- **Crash** : relire `lastprice` du mock, `movedPrice(price, 1200n)` (+12 % en bigint : `price × 11200 / 10000`), puis `set_price(Asset::Other("MXN"), nouveauPrix)` sur `config.mockOracleId`.
- **« Rafraîchir le cours »** : même chose avec le prix actuel inchangé (nouvel horodatage).
- **« Remettre le cours réel »** : `lastprice` sur Reflector → `set_price` sur le mock avec ce prix.
- Signature : **l'admin du mock = fx-admin** (`config.issuer`, même adresse). `prepareSetPrice(adresse, devise, prix)` construit et simule la transaction ; `sendSigned(xdr, signTransaction)` la fait signer par le wallet (ta fonction de `shared/wallet.ts`), l'envoie et attend la confirmation ; `txLink(hash)` → lien stellar.expert. Désactiver les boutons si le compte connecté n'est pas `config.issuer` (« Réservé au compte de démo admin »).
- Affichage après coup : « Peso : 0,0550 $ → 0,0616 $ (+12 %) » (`fmtPrice14`, `fmtPctBps(deltaBps(avant, après))`) + lien de la transaction.

**Règles de la démo** (fixing à l'échéance, facteur 25 920 → échéance réelle 5 min) :
- le crash ne compte que s'il est posé **avant l'échéance réelle** : `settle` prend le cours en vigueur à l'échéance ;
- **rafraîchir le cours juste avant `open_hedge`** (moins de 15 min) ; inutile avant `settle` ;
- après le crash, l'offre DEX MXN/USDC (0,055) n'est pas réalignée automatiquement : `docs/payment-cli.md` § 5.
- **Secours** : `scripts/crash.sh` fait la même chose en ligne de commande (`--reset` = cours réel, `--refresh`, `12` = +12 %, `--dry-run` pour vérifier sans envoyer).

## 6. Helpers de format (dans le fichier du § 7)

- `fmtPrice14(5500000000000n)` → `0,0550` ; `fmtPrice14(v, 6)` pour 6 décimales
- `fmtAmount7(550128136866n)` → `55 012,81` ; `fmtAmount7(v, 0)` → `55 013` (séparateur : espace fine insécable U+202F)
- `parseAmount7('1 000 000')` → `10000000000000n` ; accepte `1000000,5` et `12.5`, refuse `1,000,000` et plus de 7 décimales (→ `null`)
- `fmtPctBps(-318n)` → `−3,18 %` ; `deltaBps(a, b)` écart relatif arrondi
- Aucun `Number` pour les montants : uniquement `bigint` (14 et 7 décimales)

## 7. Fichier prêt à copier : `front/src/pricing/pricing.ts`

```ts
// [FRONT — Hugo] (logique fournie par Pricing) Lecture des cours, devis, erreurs, crash. Voir docs/memo-front-pricing.md.
import { Account, BASE_FEE, Contract, TransactionBuilder, nativeToScVal, rpc, scValToNative, xdr } from '@stellar/stellar-sdk'
import { config } from '../shared/config'

// ---------- simulation (lecture sans wallet) ----------

const server = new rpc.Server(config.rpcUrl)

/** Erreur d'un appel simulé, avec le code du contrat quand il y en a un (Error(Contract, #302) → 302). */
export class ContractCallError extends Error {
  readonly code: number | null
  constructor(raw: string) {
    super(raw)
    const m = /Error\(Contract, #(\d+)\)/.exec(raw)
    this.code = m ? Number(m[1]) : null
  }
}

/** Simule un appel : aucune signature, aucun frais. La source n'a qu'à exister sur le réseau. */
export async function simulate(contractId: string, method: string, args: xdr.ScVal[]): Promise<xdr.ScVal> {
  const tx = new TransactionBuilder(new Account(config.issuer, '0'), {
    fee: BASE_FEE,
    networkPassphrase: config.networkPassphrase,
  })
    .addOperation(new Contract(contractId).call(method, ...args))
    .setTimeout(30)
    .build()
  const sim = await server.simulateTransaction(tx)
  if (rpc.Api.isSimulationError(sim)) throw new ContractCallError(sim.error)
  if (!sim.result) throw new ContractCallError('simulation sans résultat')
  return sim.result.retval
}

// ---------- encodage des arguments ----------

/** `Asset::Other(Symbol)` du SEP-40 (Reflector et mock) : vec [Symbol("Other"), Symbol(devise)]. */
export const assetOther = (currency: string): xdr.ScVal =>
  xdr.ScVal.scvVec([xdr.ScVal.scvSymbol('Other'), xdr.ScVal.scvSymbol(currency)])

export type Side = 'BuyCurrency' | 'SellCurrency'
/** Enum Rust sans données (`Side`) : vec [Symbol(variante)]. */
export const sideScVal = (side: Side): xdr.ScVal => xdr.ScVal.scvVec([xdr.ScVal.scvSymbol(side)])

// ---------- décodage ----------

function bigintField(o: unknown, key: string): bigint {
  const v = (o as Record<string, unknown> | null)?.[key]
  if (typeof v !== 'bigint') throw new Error(`champ ${key} manquant ou non entier`)
  return v
}

// ---------- 1. spot ----------

export type PriceData = { price: bigint; timestamp: bigint } // price : 14 décimales, timestamp : secondes Unix

/** `lastprice(Asset::Other(currency))` : null si l'oracle n'a pas de prix pour cette devise. */
export async function readSpot(oracleId: string, currency: string): Promise<PriceData | null> {
  const retval = await simulate(oracleId, 'lastprice', [assetOther(currency)])
  const v: unknown = scValToNative(retval) // Option::None → null
  if (v === null || v === undefined) return null
  return { price: bigintField(v, 'price'), timestamp: bigintField(v, 'timestamp') }
}

/** Le contrat refuse un prix de plus de 900 s (erreur 302). */
export const MAX_PRICE_AGE_SECS = 900n
export const priceAgeSecs = (p: PriceData, nowSecs = BigInt(Math.floor(Date.now() / 1000))): bigint =>
  nowSecs - p.timestamp

// ---------- 2. devis ----------

export type Quote = {
  spot: bigint // 14 décimales
  forward_rate: bigint // 14 décimales : prix figé par unité de devise
  usdc_cost: bigint // 7 décimales : ce que la PME paiera
  margin: bigint // 7 décimales : dépôt de garantie
}

export async function quote(
  currency: string,
  side: Side,
  notional: bigint, // 7 décimales
  durationSecs: number, // durée ANNONCÉE (90 j = 7_776_000), ≥ 1
  capBps: number, // 1000 = ±10 %
): Promise<Quote> {
  const retval = await simulate(config.poolId, 'quote', [
    xdr.ScVal.scvSymbol(currency),
    sideScVal(side),
    nativeToScVal(notional, { type: 'i128' }),
    nativeToScVal(BigInt(durationSecs), { type: 'u64' }),
    nativeToScVal(capBps, { type: 'u32' }),
  ])
  const v: unknown = scValToNative(retval)
  return {
    spot: bigintField(v, 'spot'),
    forward_rate: bigintField(v, 'forward_rate'),
    usdc_cost: bigintField(v, 'usdc_cost'),
    margin: bigintField(v, 'margin'),
  }
}

// ---------- 3. montants dérivés (mêmes formules que le contrat) ----------

const E14 = 10n ** 14n
/** Division entière arrondie vers −∞ (comme `div_euclid` côté Rust pour un diviseur positif). */
const floorDiv = (a: bigint, b: bigint): bigint => (a % b === 0n || a >= 0n ? a / b : a / b - 1n)
/** `pricing::to_usdc` : notional (7 déc.) × prix (14 déc.) → USDC (7 déc.), arrondi vers le bas. */
export const toUsdc = (notional: bigint, rate: bigint): bigint => (notional * rate) / E14

/** Après un mouvement du cours : coût sans protection et coût réel avec (= `hedge::pnl`, borné à ± la marge). */
export function afterMove(q: Quote, notional: bigint, side: Side, newSpot: bigint) {
  const diff = side === 'BuyCurrency' ? newSpot - q.forward_rate : q.forward_rate - newSpot
  let pnl = floorDiv(notional * diff, E14)
  if (pnl > q.margin) pnl = q.margin
  if (pnl < -q.margin) pnl = -q.margin
  const withoutProtection = toUsdc(notional, newSpot)
  return { withoutProtection, pnl, withProtection: withoutProtection - pnl }
}

/** Coût de la protection (README, « Ce que paie la PME ») : spread + rendement perdu sur la marge. */
export const SPREAD_BPS = 50n // = SPREAD_BPS de scripts/setup-testnet.sh
export const USD_RATE_BPS = 360n // = set_rate USD 360 (scripts/rates.snapshot)
export function protectionCost(q: Quote, durationSecs: number) {
  const spread = (q.usdc_cost * SPREAD_BPS) / (10_000n + SPREAD_BPS)
  const lostYield = (q.margin * USD_RATE_BPS * BigInt(durationSecs)) / (10_000n * 31_536_000n)
  const total = spread + lostYield
  return { spread, lostYield, total, bpsOfCost: (total * 10_000n) / q.usdc_cost }
}

// ---------- 4. erreurs → messages ----------

export const ERROR_MESSAGES: Record<number, string> = {
  // Pricing (300-312)
  300: 'Cours indisponible pour cette devise',
  301: 'Cours invalide, réessayez plus tard',
  302: 'Cours trop ancien : rafraîchissez le cours',
  303: 'Devise pas encore configurée',
  304: 'Échéance maximale : 1 an',
  305: 'Montant invalide',
  306: 'Devise non disponible',
  307: 'Protection trop faible pour cette devise', // compléter avec le minimum : MIN_CAP_BPS ci-dessous
  308: 'Protection maximale : 50 %',
  312: 'Cours de règlement indisponible',
  // Token USDC (SAC) : solde insuffisant (marge de open_hedge, dépôt LP)
  10: 'Solde USDC insuffisant',
  // Vault, tranches
  111: 'Dépôt impossible pour l’instant : la part junior du pool est trop faible',
  112: 'Vous n’avez pas assez de parts senior',
}
/** Plafonds minimum posés par setup-testnet.sh (set_min_cap), pour le message 307. */
export const MIN_CAP_BPS: Record<string, number> = { MXN: 500, BRL: 700, ARS: 1500, EUR: 300 }

export function errorMessage(e: unknown, currency?: string): string {
  if (e instanceof ContractCallError && e.code !== null) {
    if (e.code === 307 && currency && MIN_CAP_BPS[currency]) {
      return `Protection minimale pour cette devise : ${MIN_CAP_BPS[currency] / 100} %`
    }
    return ERROR_MESSAGES[e.code] ?? `Erreur du contrat (code ${e.code})`
  }
  return 'Réseau indisponible, réessayez'
}

// ---------- 5. formats ----------

const NNBSP = '\u202f' // espace fine insécable, séparateur de milliers français

function groupThousands(n: bigint): string {
  const s = n.toString()
  return s.replace(/\B(?=(\d{3})+(?!\d))/g, NNBSP)
}

/** Prix 14 décimales → « 0,0550 » (arrondi au plus proche, 4 décimales par défaut). */
export function fmtPrice14(v: bigint, digits = 4): string {
  const unit = 10n ** BigInt(14 - digits)
  const r = (v + unit / 2n) / unit // v > 0
  const s = r.toString().padStart(digits + 1, '0')
  return `${s.slice(0, -digits)},${s.slice(-digits)}`
}

/** Montant 7 décimales → « 55 012,81 » (arrondi au centime le plus proche). */
export function fmtAmount7(v: bigint, digits = 2): string {
  const neg = v < 0n
  const a = neg ? -v : v
  const unit = 10n ** BigInt(7 - digits)
  const r = (a + unit / 2n) / unit
  const scale = 10n ** BigInt(digits)
  const int = groupThousands(r / scale)
  const frac = digits > 0 ? ',' + (r % scale).toString().padStart(digits, '0') : ''
  return (neg ? '-' : '') + int + frac
}

/** Saisie utilisateur (« 1 000 000 », « 1000000,50 », « 12.5 ») → bigint 7 décimales ; null si invalide. */
export function parseAmount7(input: string): bigint | null {
  const s = input.replace(/[\s\u00a0\u202f]/g, '').replace(',', '.')
  const m = /^(\d+)(?:\.(\d{0,7}))?$/.exec(s)
  if (!m) return null
  return BigInt(m[1]) * 10_000_000n + BigInt((m[2] ?? '').padEnd(7, '0'))
}

/** Écart relatif en bps, arrondi : (b − a) / a. Pour « −3,18 % ». */
export const deltaBps = (a: bigint, b: bigint): bigint => {
  const d = (b - a) * 10_000n
  return d >= 0n ? (d + a / 2n) / a : (d - a / 2n) / a
}
export function fmtPctBps(bps: bigint): string {
  const sign = bps < 0n ? '−' : '+'
  const a = bps < 0n ? -bps : bps
  const frac = (a % 100n).toString().padStart(2, '0').replace(/0+$/, '')
  return `${sign}${a / 100n}${frac ? ',' + frac : ''} %`
}

/** Crash : nouveau prix = prix × (10000 + bps) / 10000, en bigint (+12 % → 1200n). */
export const movedPrice = (price: bigint, bps: bigint): bigint => (price * (10_000n + bps)) / 10_000n

// ---------- 6. CrashButton : set_price sur le mock, signé par l'admin du mock (fx-admin) ----------

/** Construit et prépare la transaction (simulation : empreinte, frais, autorisation). Ne signe rien. */
export async function prepareSetPrice(adminAddress: string, currency: string, price: bigint): Promise<string> {
  const source = await server.getAccount(adminAddress)
  const tx = new TransactionBuilder(source, { fee: BASE_FEE, networkPassphrase: config.networkPassphrase })
    .addOperation(
      new Contract(config.mockOracleId).call('set_price', assetOther(currency), nativeToScVal(price, { type: 'i128' })),
    )
    .setTimeout(60)
    .build()
  const prepared = await server.prepareTransaction(tx) // lève une erreur si la simulation échoue
  return prepared.toXDR()
}

/** Signe avec le wallet (fonction de shared/wallet.ts), envoie, attend la confirmation. Retourne le hash. */
export async function sendSigned(xdrToSign: string, signTransaction: (xdr: string) => Promise<string>): Promise<string> {
  const signed = TransactionBuilder.fromXDR(await signTransaction(xdrToSign), config.networkPassphrase)
  const sent = await server.sendTransaction(signed)
  if (sent.status === 'ERROR') throw new ContractCallError(sent.errorResult?.toXDR('base64') ?? 'envoi refusé')
  const done = await server.pollTransaction(sent.hash, { attempts: 20 })
  if (done.status !== rpc.Api.GetTransactionStatus.SUCCESS) throw new ContractCallError(`transaction ${done.status}`)
  return sent.hash
}

export const txLink = (hash: string) => `https://stellar.expert/explorer/testnet/tx/${hash}`
```

## 8. Vérification (25/10/2026, 21:59 UTC)

Le fichier ci-dessus (avec un `config` lu depuis `.env.testnet`) a tourné sous Node 24 contre le testnet, et passe `tsc` (options de `front/tsconfig.app.json`, plus `strict`) et `oxlint` sans remarque. Aucune transaction envoyée.

```
Vérifié le 2026-10-25T21:59:24.449Z
RPC https://soroban-testnet.stellar.org
pool CAQY7OYOJTXBAJ6SSUC72EUGWBDDPFENPIDQMCUJZ35YKXUNR7V6YGRK
mock CARFDCPIC3CYMSGUSONWLS7WDSCMARHNZP4XKMJODGKKHWWCOA7657M3
reflector CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W

Cours réel (Reflector) : 1 MXN = 0,0533 $  (brut 5325247789639, ts 1792965300, âge 265 s)
Cours de démo : 1 MXN = 0,0550 $  (brut 5500000000000, ts 1792961172, âge 4393 s → PÉRIMÉ, quote renverra 302)
Écart démo / réel : +3,28 %
lastprice GBP sur le mock : null

quote(MXN, BuyCurrency, 1 000 000, 90 j, 10 %) :
  échec, code 302 → « Cours trop ancien : rafraîchissez le cours »
quote(MXN, cap 400, notional 10000000000000) → code 307 → « Protection minimale pour cette devise : 5 % »
quote(GBP, cap 1000, notional 10000000000000) → code 306 → « Devise non disponible »
quote(MXN, cap 5001, notional 10000000000000) → code 308 → « Protection maximale : 50 % »
quote(MXN, cap 1000, notional 0) → code 305 → « Montant invalide »

Valeurs de référence (taux du snapshot USD 360 / MXN 555) :
OK  fmtPrice14(spot) : 0,0550
OK  fmtPrice14(forward, 6) : 0,055013
OK  fmtAmount7(usdc_cost) : 55 012,81
OK  fmtAmount7(usdc_cost, 0) : 55 013
OK  fmtAmount7(margin) : 5 501,28
OK  movedPrice(+12 %) : 6160000000000
OK  sans protection : 61 600,00
OK  avec protection : 56 098,72
OK  protection versée : 5 501,28
coût de la protection : spread 273,70 + rendement perdu 48,83 = 322,53 USDC (+0,58 %)
OK  parseAmount7("1 000 000") : 10000000000000
OK  parseAmount7("1000000,5") : 10000005000000
OK  parseAmount7("12.5") : 125000000
OK  parseAmount7("1,000,000") : null
OK  parseAmount7("0,12345678") : null
OK  fmtPctBps(-318) : −3,18 %
OK  fmtPctBps(1200) : +12 %

set_price préparé (non signé, non envoyé) : 0,0550 → 0,0616 +12 %, XDR 768 caractères
« Remettre le cours réel » préparé : 0,0533, XDR 768 caractères
```

- `lastprice` Reflector et mock : décodés. Le mock avait 73 min : `quote` renvoie **302**, comme attendu (rafraîchir le cours avant tout devis).
- Le décodage d'un devis réussi n'a donc pas tourné en direct ce soir ; les valeurs de référence (`5501281368668` / `550128136866` / `55012813687`, taux du snapshot) ont été vérifiées sur le pool redéployé le 25/10 (`docs/todo-pricing.md`, « État »). Pour revérifier : remettre le mock à 0,055 (`./scripts/crash.sh --demo`, ou `set_price` 5500000000000 ; `--refresh` repose le cours actuel, qui n'est pas forcément 0,055 sur le testnet), puis, dans les 15 min, `quote` depuis le front ou la CLI (`docs/payment-cli.md`) : on doit retrouver ces trois valeurs à l'unité.
- `set_price` (crash +12 % et « cours réel ») : construit et simulé avec fx-admin comme source, **non signé, non envoyé**.
