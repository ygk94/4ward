# 4ward

🇫🇷 Version française : [README.fr.md](README.fr.md)

> **Lock in today the price of a supplier invoice payable in foreign currency in 90 days. On the due date, your supplier is paid in their currency, at the promised rate.**

**Live demo: [4ward-fx.vercel.app](https://4ward-fx.vercel.app)** (testnet, Freighter wallet) · **Pitch deck: [docs/4ward-pitch-deck.pdf](docs/4ward-pitch-deck.pdf)**

HackMeridian project (Stellar, Lisbon, 25-26/10/2026), **Genesis** path, tracks:
- **Stellar Unlocked**: *strict receive* path payment USDC → MXN (the supplier receives the exact amount), USDC as a SAC, Soroban events, `require_auth`, entry TTLs;
- **Stellar Composed**: Reflector FX oracle (SEP-40), Stellar Wallets Kit (Freighter and other wallets), Etherfuse rates (CETES, USTRY, TESOURO, EUROB) in the forward price.

**Live on testnet** ([pool contract on stellar.expert](https://stellar.expert/explorer/testnet/contract/CACSMEBQLBR73CH6BRPGY7LDDE2P63C6WPZEJPZNIP5DLT5IN6O5VWA4)): forward quote and hedge opening (`quote`, `open_hedge`), settlement at the maturity fixing (`settle`), early close at market price (`close_early`: the losing side pays, exit fee = spread), supplier payment via path payment, two-tranche LP vault with capacity rule, peso crash on the demo oracle, settlement keeper. **Roadmap**: tranche yield from treasury, netting, pool hedging, atomic payment.

---

## The problem, as a story

Lucía imports tequila in Lisbon. She owes **1,000,000 pesos** to her supplier in Guadalajara **in 90 days**. If the peso rises 10% by then, her invoice costs her $5,500 more, and her margin is gone.

A large company would call its bank to lock the rate (an *FX forward*). Lucía has no credit line, and her bank will never offer her this product. On Stellar, she can pay abroad in seconds, but **only at spot**. Nothing lets her lock a rate for a future payment.

## The solution

1. Lucía enters her invoice: *1,000,000 MXN in 90 days*.
2. The app shows her a locked price: *"you will pay 55,013 USDC"*, as long as the peso moves less than the protection she chose (beyond that, she only bears the part of the move that exceeds her cap).
3. She picks her protection (e.g. ±10%), deposits the matching margin in USDC and signs with her wallet (Freighter, via Stellar Wallets Kit).
4. On the due date, anyone can trigger settlement. The contract takes the rate **in effect at maturity** from a SEP-40 oracle (`price(asset, timestamp)`; Reflector FX in production, a mock-oracle with the same interface for the demo), not the rate at the moment of settlement:
   - the peso went up → the pool pays her the difference;
   - the peso went down → the difference is taken from her margin.
5. Right after, a **Stellar path payment** converts her USDC into pesos and pays the supplier: **exactly 1,000,000 MXN**, for a total cost equal to the price locked on day 0.

**Why Stellar:** the hedge and the supplier's foreign-currency payment run on the same rail: native USDC, Reflector FX oracle, path payment all the way to the peso. No bank, no credit line, and a settlement anyone can trigger.

---

## How it works

### The actors

| Actor | Role |
|---|---|
| **SME** (Lucía) | Locks the price of a future invoice, deposits a margin |
| **LP** | Deposits USDC into the pool, acts as counterparty, earns the spreads |
| **Pool contract** (Soroban) | Holds the funds, records the hedges, enforces the capacity rule |
| **Reflector oracle** (SEP-40) | Rate of each currency against USD |
| **Stellar DEX / anchor** | Converts USDC into local currency to pay the supplier |

### The locked price

The locked price is not today's rate but the **forward rate**: today's rate adjusted for the interest-rate differential between the two currencies, plus the pool's spread.

```
Spot and F in USD per unit of currency (Reflector oracle convention):
F = Spot × (1 + r_USD × t) / (1 + r_currency × t)      quoted price = F ± spread
```

Example: 1 MXN = 0.055 USD, r_MXN = 5.55%, r_USD = 3.6%, 90 days → F ≈ 0.05474 USD (0.05501 including spread). The peso, which pays a higher rate, trades cheaper forward. Without this adjustment, arbitrageurs could make risk-free profits against the pool.

`r_USD` is the rate of tokenized US Treasury bills (3.6%: Etherfuse USTRY): it is what the pool's capital would earn if it were not used for hedging (see *Where the pool's money sits*).

Rates are **read live from the public Etherfuse API** (`current_basis_points`: USD ← USTRY, MXN ← CETES, BRL ← TESOURO, EUR ← EUROB; ARS hardcoded) **and pushed to the pool by the admin** (`scripts/sync-rates.sh`, `--loop 3600` for hourly sync). The contract does not read Etherfuse itself. Under each quote, the app shows the current Etherfuse rates and the rate actually used by the pool, recovered by inverting the formula on the quote (no rate getter); beyond a 5 bps gap, it shows "pool rate out of sync". `scripts/rates.snapshot` is now only used for setup and as a fallback if the API does not respond.

In production, the spot comes from **Reflector FX**, Stellar's native FX oracle (SEP-40, updated every 300 s). On testnet, the demo reads a mock-oracle with the same interface, so we can trigger the peso crash (see *Architecture*). Outside of filming, `scripts/keepalive.sh --live` copies the Reflector FX testnet rate into the mock every 10 min and realigns the MXN DEX offer (`scripts/dex-align.sh`): the public site quotes at today's rate. The contract always reads the mock, fed by our script: this is not a direct read of Reflector. On mainnet (`CBKGPWGKSKZF52CFHMTRR23TBWTPMRDIYZ4O2P5VS65BMHYH4DXMCJZC`), it covers **24 currencies** (verified onchain on 25/10/2026): EUR, GBP, CAD, JPY, CNY, HKD, KRW, INR, PHP, TRY, RUB, ZAR, MXN, BRL, ARS, COP, CLP, PEN, CRC, VES, NGN, KES, CDF, XAU.

### Capped protection, no default risk

The margin chosen by the SME sets both **its maximum loss** and **its maximum protection**:

| Margin deposited | Protected if the currency moves by |
|---|---|
| 5% (minimum, higher on unstable currencies) | ±5% |
| 10% | ±10% |
| 20% | ±20% |

Everything that can be lost is **already deposited** in the contract: no margin call, no liquidation, no credit risk. Technically, it is an **FX collar** centered on the forward rate: its value is near zero at opening, and the spread pays the pool.

> *"But what if the currency moves 15%?"* The SME chooses its cap. And its real alternative is not a free bank forward, it is **no hedge at all**: the locked margin is the price of a hedge without a bank.

### What the SME pays

For 1,000,000 MXN at 90 days, 10% cap, 0.5% spread (≈ 55,013 USDC):

| Cost | Amount | % of invoice |
|---|---|---|
| Pool spread | ~274 USDC | 0.50% |
| Yield forgone on the locked margin (5,501 USDC at ~3.6%) | ~49 USDC | 0.09% |
| Forward / spot gap | — | not a fee: it is the fair price (in favor of a peso importer) |
| Stellar fees, oracle | ~0 | ~0 |
| **Total** | **~323 USDC** | **~0.6%** (≈ 2.4%/year) |

The USDC → MXN conversion to pay the supplier is not counted: it is owed with or without a hedge.

| Solution | Cost for 90 days | Access for an SME |
|---|---|---|
| No hedge | 0, but a 6.5% move (common over 90 d) costs ~$3,600 | — |
| **4ward** | **~0.6%** | no bank or credit line, margin of 5 to 20% of your choice |
| Bank forward | ~0.5 to 2% | credit line, rarely granted to a small SME |
| FX fintechs (Ebury, iBanFirst, Corpay) | ~0.5 to 1.5% | 5 to 10% initial deposit, KYC, limited currencies |
| Call option (uncapped protection) | ~2.6% (at-the-money premium, 13% vol) | rare for an SME |

About **4 times cheaper than an option**, because the protection is capped in both directions; at the price of a forward, but without a bank. Accepted trade-off: beyond the cap, the SME is no longer protected. *(Banks and fintechs: market orders of magnitude, not verified currency by currency.)*

### The pool (vault)

**Dollars only.** The pool holds only USDC. Each hedge is a "currency against dollar" leg: the pool never needs to store pesos.

**Shares, like a fund.** An LP depositing USDC receives **shares**. The first deposit is at 1 share = 1 USDC. When the pool gains (the spread, or a hedge that turns in its favor), each share is worth more. When it loses, each share is worth less. Nobody needs to allocate gains account by account.

**Two prices per share:**

| Price | Calculation | Used to |
|---|---|---|
| full price | junior value ÷ junior shares | enter (deposit) |
| exit price | (junior value − reserved capital) ÷ junior shares | exit (withdrawal) |

Reserved capital is the maximum loss of open hedges. It is not paid out to whoever exits. So nobody can leave and dump their potential losses on the others. Withdrawals are fair by construction: each LP can only withdraw its share of the free capital. If the hedge ends well, the discount left by the departing LP benefits the remaining LPs.

**Capacity rule: the pool's FX risk is capped and covered by junior capital.** The maximum loss of each hedge (its margin) is known at opening:

```
reserved capital = Σ margins of open hedges   ≤   junior tranche capital (Dynamic)
```

The contract **rejects any hedge** that would exceed this limit. SME margins are deposited in the contract but do not belong to the LPs. Tests check on every operation that `contract USDC balance = LP capital + margins of open hedges`.

**Exposure and per-currency limit.** For each currency, the pool tracks buyer volume and seller volume (in USDC at the locked rate), as well as their margins. The admin can cap a currency, e.g. "ARS margins do not exceed 10% of capital". A limit of 0 suspends new hedges in that currency.

**Robustness.** Storage entries are extended automatically (120 days). Deposits, withdrawals, openings, closings and limit changes emit events readable on the explorer. All calculations are overflow-protected and rounded in the pool's favor.

### Netting (calculation ready, wiring on the roadmap)

Lucía fears the peso will **rise**. Her natural counterparty fears it will **fall**: a Mexican importer paying its suppliers in dollars, or a company paid in pesos. When one wins, the other loses: in theory, only the net exposure should consume capital.

The contract already contains the **exact calculation** of the maximum loss of a basket of hedges, over all possible rates, even with different caps and locked rates. It is proven by property tests (moves from −30% to +30%, 50 random baskets, counterexamples). A naive "net of margins" would be wrong: with different caps, or rates locked on different dates, it underestimates the loss.

The MVP keeps the gross rule, which is more conservative. Netting will be wired in once three conditions are met:
1. a common fixing at maturity;
2. maturities rounded to fixing dates;
3. settlement per basket, or accounting for what the opposite leg owes.

### LPs: senior tranche and junior tranche

LPs deposit into the same pool but choose their risk:

| | **Senior** (conservative) | **Junior** (risk) |
|---|---|---|
| Profile | Wants a stable yield | Accepts losses to earn more |
| Their money is used to | Hedge the exposure (CETES, EURC) + treasury (roadmap; today it stays as USDC in the contract) | Absorb the unhedged exposure |
| Losses | Protected by the junior | Loses first |
| Yield | Target rate paid first (4.5%, not guaranteed) | Everything else: spread, client losses, treasury |

Contract rules:
1. **Max unhedged loss ≤ junior capital**: the capacity rule applies to the junior only. Since each contract is capped, the senior cannot lose because of FX (it remains exposed to third-party protocols and to USDC).
2. **Waterfall**: losses hit the junior first; gains first pay the senior rate, the junior takes the surplus.
3. **The hedged share is a setting** (roadmap, with pool hedging): more junior, less hedging and more leverage.
4. **Junior depleted**: zero capacity, every new hedge is rejected (#101). On the roadmap: only accept fully hedged hedges.
5. **Minimum ratio**: as long as there are seniors, the junior stays ≥ 20% of capital at the time of each senior deposit or junior withdrawal (rejected otherwise); senior interest can push it lower afterwards.

**The senior's 4.5% is a target rate set by the contract, not an earned yield: it is paid by the junior as long as free capital is not invested** (Treasury bills, CETES): the senior share price grows by 4.5%/year, and this growth is deducted from the junior value. Treasury investment is on the roadmap; until then, the junior pays the senior out of its spreads.

*Implemented in the contract (`main`, deployed on testnet)*: a single vault, two share classes. `deposit_lp` / `withdraw` = junior (unchanged), plus `deposit_senior` / `withdraw_senior` / `senior_balance` / `tranche_values`. Senior price compounded on each senior operation, junior value = capital − senior value, capacity = junior − reserved.

Simulation of a starting pool: **$500k** ($400k senior paid 4.5%, $100k junior used at 15% max), **$10k contracts** at 90 days with a random direction (importer / exporter); peso at 13% vol with 15% jumps; spread 0.4% per 90 d (conservative assumption; the contract charges 0.5%) of which 20% goes to the protocol; 8% in USDC, the rest in Treasury bills at 4%; no Blend.

| Contracts / quarter | Volume / quarter | Share of importers | Volume netted | Pool | **Junior (average)** | Losing years | Worst year out of 20 |
|---|---|---|---|---|---|---|---|
| 20 | $200k | 50% | 82% | 4.3% | 3.7% | 22% | −5% |
| 20 | $200k | 70% | 59% | 4.4% | 3.8% | 35% | −13% |
| 40 | $400k | 50% | 87% | 5.0% | 7.1% | 15% | −5% |
| 40 | $400k | 70% | 60% | 5.0% | 6.9% | 34% | −19% |
| **80** | **$800k** | 50% | 91% | 6.4% | **13.8%** | 7% | −2% |
| **80** | **$800k** | 60% | 80% | 6.3% | **13.6%** | 19% | −11% |
| 80 | $800k | 70% | 60% | 6.3% | 13.3% | 23% | −16% |

Benchmark: treasury alone (92% at 4%) yields ~3.7%; the senior earns 4.5%.

Key takeaways:
- **The junior's yield comes from volume** (the spread is taken on each contract), its **risk comes from imbalance** between importers and exporters. With $10k contracts, it takes **~80 contracts per quarter** (≈ 27 per month, 1.6 times the pool turned over each quarter) for the junior to exceed 13%. At 20 contracts per quarter, it earns less than Treasury bills.
- **Many small contracts net well when both directions exist**: at 50/50, 80 to 90% of the volume nets out on its own. But with 70% importers, ~40% of the volume stays unnetted regardless of the number of contracts: size does not replace two-way demand (price skew, distribution to exporting SMEs).
- Junior utilization stays capped at 15-20%: beyond that, a near-total loss becomes possible.

### Where the pool's money sits (roadmap)

Three buckets, in the order the pool draws on them to pay:

| Bucket | Size | Yield | Role |
|---|---|---|---|
| USDC in the contract | **8% of deposits** | 0% | Same-day settlements and LP withdrawals, without relying on a third party |
| Tokenized US Treasury bills (BENJI, Etherfuse USTRY… to be verified) | the rest | ~4% | Stable yield, redemption within one day at most |
| CETES / EURC | based on net exposure | pricing `r_USD` rate | The hedge |

Example, $500k pool at 80 contracts per quarter (60% importers): ~$160k net exposure, of which $150k carried by the junior and ~$10k hedged in CETES. Treasury: $40k in USDC, ~$530k in Treasury bills (LP deposits + ~$80k of client margins).

**No Blend in the base model**: it would add ~0.5 point, but adds a third-party protocol (liquidity limited to ~$14M withdrawable, oracle exploit on 22/02/2026). It remains a roadmap option, capped at 20%. A `rebalance()` function brings the buckets back to target: new hedge → buy CETES; maturity approaching → move back to the USDC reserve.

### Pool hedging (roadmap)

The cap and the capacity rule guarantee that the pool's FX losses stay **capped and covered by junior capital**, not that it is consistently profitable. The pool does not bet on the trend (the forward rate already prices it in), but it remains exposed to currency moves: about 6.5% over 90 days on the peso, against a 0.5% spread. Without hedging, a 3 to 5% spread would be needed to pay LPs, and no SME would buy. So the pool hedges in three layers, like a bank selling a forward.

```
Peso exposure
 ├─ 1. Netting: clients on opposite sides offset each other       → free
 ├─ 2. The net remainder: the pool holds the currency (≈ 88% of net),
 │      invested in tokenized government bonds                   → nearly free
 └─ 3. What cannot be hedged: per-currency limits + capacity rule
```

**1. Netting.** Lucía owes pesos: she fears the peso will **rise**. Her natural counterparty fears it will **fall**: a **Mexican importer** paying its suppliers in dollars, or an exporter paid in pesos. What the pool pays one, it receives from the other, and it keeps both spreads. Only the net exposure consumes capital. A price **skew** (the over-demanded side pays more, the other less) attracts the missing side. *Note: a Mexican exporter receives dollars and also fears the peso will rise, so it is on the same side as Lucía.*

**2. Holding the currency.** If the pool must pay when the peso rises, it buys pesos now: if they rise, they offset what it has to pay. These pesos are invested at the local rate, in **tokenized CETES** (Mexican government bonds issued on Stellar by Etherfuse, to be verified). This local yield pays exactly the gap between the forward rate and spot (interest rate parity): the hedge only costs conversion fees, and the pool keeps the spread.

| Currency | Hedge | Feasibility |
|---|---|---|
| EUR | Native EURC | ✅ |
| MXN | Tokenized CETES (Etherfuse) | ✅ to be verified (liquidity, access) |
| BRL, ARS, NGN… | Anchor stablecoins | ⚠️ depending on liquidity |
| Currencies without a token | Partner market maker, otherwise tight limits and a higher minimum cap | ⚠️ later |

**3. Limiting the rest.** Net exposure cap per currency; a currency is only opened if it is hedgeable or if its flows offset; the capacity rule remains the last safety net.

**The hedge is not complete.** It neutralizes most of the FX risk. The remainder is small, measurable, and paid for by the spread:
- **Capped protection, uncapped hedge**: beyond the cap, the currency position no longer matches (gain if the peso rises 15%, loss if it falls 15%). The position must be reduced as the rate approaches the cap, and a sudden jump can fall between two adjustments.
- **Bonds**: price sensitive to rates if sold before maturity (hence 28-day CETES), token issuer risk, liquidity at settlement time.
- **Price gaps**: oracle at fixing vs execution price on the DEX, hardcoded rates vs actual CETES yield, lag between contract opening and hedge purchase.

**Hedge the portfolio, not each contract.** Buying and selling CETES on every hedge would cost ~2% per year in conversions and eat the entire spread. So the pool holds **a single position per currency**:
- the target is computed continuously (≈ 88% of net exposure across all open hedges);
- the pool only trades the gap with the target, and only beyond a threshold (e.g. 10%, or once a day);
- a maturing contract replaced by a new one, or two contracts in opposite directions, trigger **no trade**;
- CETES are rolled at maturity without going back through USDC;
- optimization: at maturity, pay the supplier **directly in the currency** from the hedge, saving two conversions.

With hedging, the spread no longer pays for risk, only for costs: see *What the SME pays*. LP yield depends mainly on **netted volume**: see *Senior tranche and junior tranche*.

The MVP shows the safety net (cap + gross capacity rule, more conservative); netting and automatic hedging are the next step.

---

## Architecture

```
┌──────────────┐  Freighter  ┌──────────────────────────────────────┐
│ Front (Vite) │────────────▶│      Pool contract (Soroban, Rust)   │
│  SME screen  │             │  deposit_lp / withdraw               │
│  LP panel    │             │  quote / open_hedge / settle         │
└──────┬───────┘             │  capacity                            │
       │                     └──────────────┬───────────────────────┘
       │ path payment                       │ lastprice / price(ts) (SEP-40)
       ▼                                    ▼
┌──────────────┐             ┌──────────────────────────────────────┐
│ Stellar DEX  │             │ Reflector FX (production)            │
│ USDC → MXN   │             │ mock-oracle SEP-40 (demo: "crash")   │
└──────────────┘             └──────────────────────────────────────┘
```

**Reflector integration.** The contract only talks to the oracle through the SEP-40 interface (`lastprice`, `price(asset, timestamp)`, `decimals`) and has been verified against Reflector FX testnet (`CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W`: 14 decimals, USD base, history and rounding to the 300 s period). The demo pool is built with a mock-oracle with the same interface, controlled by the admin, to trigger the peso crash during the video; in production, Reflector's address is passed to the constructor, with no code change.

Errors: Vault 100-112, Payment 200-206, Pricing 300-312, USDC token #10 (insufficient balance); full table: `docs/payment-cli.md` (§ Erreurs à traduire).

A Soroban transaction cannot contain a classic operation: the front chains `settle()` then the path payment, in two transactions.

### Contract interface

```rust
// Vault (amounts and shares in 7 decimals)
fn deposit_lp(lp: Address, amount: i128) -> i128;        // USDC deposited → LP shares
fn withdraw(lp: Address, amount: i128) -> i128;          // shares burned → USDC paid out (exit price)
fn lp_balance(lp: Address) -> i128;                      // value of shares at exit price
fn lp_shares(lp: Address) -> i128;
fn share_price() -> i128;                                // full price, USDC per share
fn exit_price() -> i128;                                 // exit price, USDC per share
fn total_assets() -> i128;                               // LP capital
fn deposit_senior(lp: Address, amount: i128) -> i128;    // senior tranche: USDC deposited → senior shares
fn withdraw_senior(lp: Address, shares: i128) -> i128;   // senior shares burned → USDC paid out
fn senior_balance(lp: Address) -> i128;                  // value of the LP's senior shares
fn tranche_values() -> TrancheValues;                    // senior, junior, senior share price
fn capacity() -> i128;                                   // unreserved capital
fn exposure(currency: Symbol) -> Exposure;               // buyers / sellers / net, margins
fn currency_limit(currency: Symbol) -> Option<u32>;
fn set_currency_limit(currency: Symbol, limit_bps: u32); // admin
// Pricing
fn quote(currency: Symbol, side: Side, notional: i128, duration_secs: u64, cap_bps: u32) -> Quote;
fn set_rate(currency: Symbol, rate_bps: u32);            // admin
fn set_min_cap(currency: Symbol, cap_bps: u32);          // admin
fn set_spread(spread_bps: u32);                          // admin
// Hedges / payment
fn open_hedge(owner: Address, supplier: Address, currency: Symbol, side: Side,
              notional: i128, duration_secs: u64, cap_bps: u32) -> u64;
fn settle(hedge_id: u64) -> i128;                        // permissionless after maturity
fn close_early(hedge_id: u64) -> i128;                   // owner only, before maturity: close at market price
fn get_hedge(hedge_id: u64) -> Option<Hedge>;
fn time_factor() -> u32;                                 // 1 = real time
fn set_time_factor(factor: u32);                         // admin, demo: 25,920 → 90 d in 5 real minutes
```

---

## Testnet deployment

| Item | ID |
|---|---|
| Pool contract | `CACSMEBQLBR73CH6BRPGY7LDDE2P63C6WPZEJPZNIP5DLT5IN6O5VWA4` |
| mock-oracle (SEP-40) | `CD2D3KQDUCE7SHQNT4XJUMQRHQNDNJUKE45TER2LSHAJPYCTRXX6NBVU` |
| Reflector FX (testnet) | `CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W` |
| Test USDC (SAC) | `CCLMFOUKC7ZQ5OPFE67F5XUGCOWTVCOOVTKTEPQGEYKWWRRKJGNVR2OC` |
| Test MXN (SAC) | `CAFK4PTVDK6Y33HWTSUPH57JV3G4YZFS6PL77FNGWOOMHBJJESYBMVMS` |

Reflector FX testnet covers: EUR, GBP, CHF, CAD, **MXN, ARS, BRL**, THB, XAU (USD base, 14 decimals, 300 s resolution).

The pool is redeployed on every `./scripts/setup-testnet.sh`, which also makes the LP deposit (1,000,000 USDC via `deposit_lp`). Current IDs are in `.env.testnet`.

**Demo mode:** setup sets `set_time_factor 25920`: the price is computed over the stated duration (90 days), but the actual maturity falls 5 minutes after opening. This is what lets us settle a "90-day" hedge live in the video.

**Testnet MVP limitations:** supplier payment from the app in MXN only (the only currency with a test asset and a DEX offer placed by the setup; BRL, ARS and EUR hedges settle normally); demo oracle (SEP-40 mock-oracle, for the crash) instead of Reflector, copied from Reflector by `keepalive.sh --live` outside of filming; test USDC and MXN issued by the ISSUER account, MXN/USDC DEX offer placed by us; gross capacity rule (sum of margins, no netting); interest rates read from the Etherfuse API and pushed by the admin (`scripts/sync-rates.sh`), not read by the contract.

## Running the project

```sh
# contracts
stellar contract build
cargo test

# full testnet environment (accounts, assets, DEX offer, contracts)
./scripts/setup-testnet.sh

# front
cd front && pnpm install && pnpm dev
```

## Structure

```
contracts/pool         # vault.rs (capital, LP shares, exposure) · pricing.rs (spot, forward rate, caps) · hedge.rs (hedges)
contracts/mock-oracle  # admin-controlled SEP-40 oracle (demo)
front/                 # Vite + React + TS, stellar-sdk, Stellar Wallets Kit
scripts/               # testnet setup and seeding, peso crash, settlement keeper, Etherfuse rate sync
sim/                   # pricing / risk simulator (optional)
docs/council/          # 25/10 council: report + transcript
TODO.md                # setup and work items
```

## Roadmap

**Right after the hackathon**
- Atomic payment: settle the hedge and pay the supplier in a single transaction (swap via a Soroban DEX)
- Productive margin: the SME's margin invested in tokenized Treasury bills during the hedge
- Robust fixing: TWAP, consistency bounds, multiple oracles

**Pool and risk**
- Netting per currency and per maturity (SMEs paying in a currency / SMEs receiving it)
- Automatic hedging of net exposure: holding the currency invested in tokenized government bonds (CETES) or EURC; one position per currency with a rebalancing threshold
- Senior tranche yield from invested treasury, LP shares as a SEP-41 token
- Treasury management: 8% USDC, tokenized Treasury bills for the rest; Blend as an option (≤ 20%, ~+0.5 pt)
- Per-currency limits, price skew, dynamic spread
- Interest rates read onchain, fixing on anchors' SEP-38 quotes

**Product and distribution**
- Direct delivery in the currency to the supplier from the hedge
- Transferable hedged invoice: sell an already-hedged receivable to a financier (factoring)
- "Lock the rate" SDK for wallets and anchors
- KYC via anchors (SEP-12) for the regulatory framework
- Audit, then mainnet

## Known risks

| Risk | Mitigation |
|---|---|
| SME default | Impossible: the max loss is already deposited |
| Pool FX losses | Capped: capacity rule, covered by junior capital (the senior remains exposed to USDC, the oracle and junior depletion) |
| LPs exposed to currency moves | Netting, then hedging the net exposure in the currency (see *Pool hedging*) |
| All flows in the same direction (adverse selection) | Per-currency limits, price skew, dynamic spread |
| Rate jump between two rebalancings | Per-currency limits, junior tranche as first loss |
| LP withdrawing before a losing settlement | Withdrawals deferred to the next maturity, or share price including open contracts |
| Third-party protocols (CETES and Treasury bill issuers, Blend if enabled) | Per-protocol cap, USDC reserve in the contract |
| Interest rates pushed by the admin | Rates read live from the Etherfuse API and pushed by the admin (`scripts/sync-rates.sh --loop`), gap shown in the app beyond 5 bps; onchain reading on the roadmap |
| Oracle manipulated at fixing | TWAP over a window, consistency bounds, multiple oracles |
| Gap between oracle and actual DEX or anchor rate | Small residual, shown to the user |

### Known contract limitations (MVP)

Fixes planned after the hackathon; details in `docs/points-a-trancher.md`.

- **Fixing unavailable (302/312) = hedge cannot be settled.** `settle` settles at the rate in effect at maturity (`pricing::fixing`). If the oracle has no price less than 900 s before maturity (302), or if maturity is beyond its history (312), the hedge can no longer be settled: the SME's margin and the reserved capital stay locked. A `settle_fallback` escape path (admin, after maturity + grace period, zero P&L) is on the roadmap.
- **Demo mode (`time_factor` > 1)**: the price covers the stated duration and maturity is shortened, so carry is not neutralized (arbitrage possible against the pool). `time_factor` must be 1 in production (eventually: `set_time_factor` behind a cargo feature).
- **Deposit just before a winning `settle`**: the fixing is known at maturity, so an LP can enter at full price just before settlement and exit right after. To be fixed with an entry delay or by blocking `deposit_lp` while a matured hedge is unsettled.
- **Tranches**: accrued senior interest lowers the junior value, which can fall below reserved capital (negative capacity) or below 20% without any operation; `withdraw_senior` does not yet check that the remaining capital covers the reserved amount.

## Competition

*FX Swap by Hedgehog* (SCF #21) offers currency swaps with physical delivery (USDC/EURC/AUDD) for DeFi users. 4ward targets SMEs: a hedge on any currency covered by the oracle, without mandatory physical delivery (the pool only holds the currency to hedge its net exposure), tied to the supplier payment.
