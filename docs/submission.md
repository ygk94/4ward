# 4ward — submission form

> Draft of every field of the HackMeridian submission form. Character counts are given for the fields that have a limit.

---

## Tagline (≤ 120 characters)

**Lock today's exchange rate for an invoice you pay in 90 days. No bank, no credit line: just a wallet on Stellar.**

*(112 characters)*

Alternatives:
- `Capped FX forwards for small businesses on Stellar: lock your supplier's price today, pay them in their currency later.` (119)
- `FX hedging for SMEs that banks ignore: lock a rate on Soroban, settle on time, pay your supplier in pesos.` (106)

---

## Description (~6 000 characters)

**4ward lets a small business lock today's price for a foreign-currency invoice it will pay in the future, and then pay its supplier in that currency on the due date, all on Stellar.**

Lucía imports tequila into Lisbon. She owes her supplier in Guadalajara **1,000,000 Mexican pesos in 90 days**. If the peso rises 10% by then, her invoice costs her about $5,500 more and her margin is gone. A large company would call its bank and buy a currency forward. Lucía has no credit line, and her bank will never offer her that product. Stellar already lets her pay abroad in seconds, but only at today's rate: nothing lets her fix a rate for a payment she will make later.

**How it works for the business**

1. Lucía enters her invoice: 1,000,000 MXN, due in 90 days.
2. 4ward shows a locked price: *"you will pay 55,013 USDC, whatever happens"*. This is the fair 90-day forward rate (today's rate adjusted for the interest-rate gap between the dollar and the peso) plus a 0.5% spread.
3. She chooses her protection level (for example ±10%), deposits the matching margin in USDC (5,501 USDC) and signs once with her wallet (Freighter or any wallet through Stellar Wallets Kit).
4. On the due date anyone can trigger settlement. The contract reads the rate **in force at maturity** from a SEP-40 oracle (Reflector FX in production), not the rate at the moment someone clicks, so nobody can game the timing. If the peso went up, the pool pays her the difference. If it went down, the difference comes out of her margin.
5. A Stellar **strict-receive path payment** converts her USDC into pesos: the supplier receives **exactly 1,000,000 MXN**, and her total cost equals the price she locked on day 0.

In our recorded scenario the peso jumps 12% before maturity. Without 4ward, Lucía pays 61,600 USDC. With 4ward she pays 56,099 USDC in total, and the pool pays out 11,002.56 USDC at settlement.

**A capped hedge with no default risk**

The margin Lucía picks is both her maximum loss and her maximum protection. Everything that can be lost is already deposited in the contract when the hedge opens: no margin calls, no liquidations, no credit risk, and no back-end monitoring prices around the clock. Financially it is a collar centred on the forward rate. Because protection is capped in both directions, it costs about 0.6% of the invoice (spread plus the yield lost on the locked margin), roughly four times cheaper than an option, and about the price of a bank forward without needing a bank.

Plans change, so Lucía can also **close early**. The contract prices her exit at the market forward for the remaining duration, on the opposite side. Whoever is losing pays, the exit fee is the spread, and the pool releases its reserve.

**The liquidity pool**

Liquidity providers deposit USDC into a Soroban vault and act as the counterparty. They earn the spreads. The vault issues shares with two prices: a full price to enter, and an exit price that excludes capital reserved for open hedges, so nobody can leave and dump potential losses on the others. A **capacity rule** makes the pool unable to go bankrupt: the sum of the maximum losses of open hedges can never exceed pool capital, and the contract refuses any hedge that would break it. LPs choose their risk through two tranches in the same vault:
- **Senior (Safe):** a fixed priority rate, protected from FX losses.
- **Junior (Dynamic):** takes the first loss and keeps the upside.

The pool also tracks exposure per currency, with admin limits per currency. After every operation the tests check the invariant *contract USDC balance = LP capital + margins of open hedges*.

**What is live on testnet**

- A single Soroban pool contract (Rust):
  - forward quotes;
  - opening, settlement at the maturity fixing, and early close of hedges;
  - the two-tranche vault with the capacity rule;
  - per-currency exposure and limits;
  - Soroban events for every action.
- Four hedgeable currencies (MXN, BRL, ARS, EUR), each with its own minimum protection level.
- Interest rates come from **Etherfuse** tokenized bonds (USTRY 3.6%, CETES 5.55%, TESOURO, EUROB).
- Prices come from a SEP-40 oracle. The contract talks only to the SEP-40 interface and was checked against **Reflector FX testnet**. For the demo we plug in a mock oracle with the same interface, so we can crash the peso live. In production, Reflector's address is passed to the constructor without any code change.
- A React front end deployed on Vercel:
  - quote and hedge screen;
  - My positions, with settle and close early;
  - the LP vault;
  - a Risk lab running a Monte-Carlo simulation of the pool in a web worker.
- Operator scripts: one-command testnet setup, a seeding script (6 businesses, 2 LPs, hedges in 4 currencies), a settlement keeper, a peso-crash script and an oracle keepalive.
- 152 contract tests, including property tests.

**Why Stellar**

The hedge and the payment in the supplier's currency run on the same rail. USDC is native, Reflector publishes FX prices on-chain, and path payments convert to pesos in a single operation. Fees are negligible, so a hedge of a few thousand dollars makes sense, which no bank offers today.

**What we deliberately left for later**

- **Netting.** Opposite clients would offset each other so that only the net exposure uses capital. The exact max-loss computation for a basket of hedges is already written and property-tested.
- **Pool hedging.** Holding the currency in tokenized government bonds such as CETES or EURC, with one position per currency and a rebalancing threshold.
- **Yield on idle treasury.**
- **Atomic settle-and-pay.**

The MVP shows the safety net: capped hedges, a gross capacity rule, and settlement at the maturity fixing. The economics of a production pool are modelled in our simulator: the junior tranche earns about 13–14% a year at roughly 80 contracts of $10k per quarter.

---

## Problem solved

**Who faces it.** Small and medium businesses that buy or sell across borders in emerging-market currencies face this problem:
- importers paying suppliers in Mexico, Brazil or Argentina;
- exporters invoicing in dollars while their costs are in local currency;
- European SMEs paying in dollars.

Their invoices are often due in 30 to 120 days. A 6–10% currency move over 90 days is common on the peso and routine on the Brazilian real or the Argentine peso. For a business running 5–10% margins, a single move can erase the profit of the whole order.

Large companies solve this with bank forwards. SMEs are mostly locked out:
- banks require a credit line and minimum sizes, and rarely offer forwards to small clients;
- FX fintechs ask for KYC, an initial deposit, and cover a limited list of currencies;
- options are expensive (about 2.6% of the invoice for 90 days).

Most SMEs therefore stay unhedged and simply absorb the risk. Crypto rails made cross-border payments fast and cheap, but only spot: nothing lets a small business fix a rate for a payment it will make later.

**How we solve it.** 4ward is a capped, fully collateralized FX forward (an NDF) on Soroban, connected to the payment itself:
- **No bank and no credit line.** The business deposits a margin between 5% and 20% of the invoice. That margin is both its maximum loss and its maximum protection, so there is no counterparty or credit risk to underwrite.
- **A fair, transparent price.** The forward rate follows interest-rate parity, using live tokenized-bond yields from Etherfuse, plus a visible 0.5% spread. Total cost is about 0.6% of the invoice.
- **Settlement nobody can game.** Settlement uses the oracle rate in force at maturity (SEP-40 `price(asset, timestamp)`), and anyone can trigger it.
- **The supplier is paid in their own currency.** A strict-receive path payment delivers exactly the invoiced amount. The business's total cost is the price it locked on day 0.
- **An exit is always possible.** Early close at the market forward, where the losing side pays.
- **Counterparty liquidity comes from an open pool.** Its solvency is enforced by the contract (capacity rule, two tranches), not by trust.

---

## Challenges

**Making a 90-day product demoable in 3 minutes.** A forward is priced on its duration. If we had opened a hedge expiring in 60 seconds, the price on screen would have collapsed to spot and lost the interest-rate gap that is the core of the product. We added an admin-only `time_factor`: the price is computed on the announced 90 days while the real maturity falls 5 minutes later. That way the video shows the real 90-day price and settles it live. We documented why `time_factor` must be 1 in production, since otherwise the carry could be arbitraged.

**Settling at the right price.** Our first version of `settle` read the oracle price at the moment someone called it. Since settlement is permissionless, whoever settles could pick their moment: a free option the pool never charged for. We moved settlement to the fixing at maturity through SEP-40 `price(asset, timestamp)`. That meant adding price history and timestamp lookup to our mock oracle so it behaves exactly like Reflector, and handling the edge cases (no price within 900 s before maturity, maturity beyond the oracle's history).

**Keeping the pool solvent by construction.** We had to make sure the vault can never owe more than it holds, and that an LP cannot withdraw just before a losing hedge settles. That led to the two share prices (entry versus exit, which excludes reserved capital), the gross capacity rule, rounding in favour of the pool everywhere, and an invariant test run after every operation. We also wrote the exact netting computation and found through property tests that the "naive net of margins" approach underestimates losses when caps or locked rates differ. That is why netting stays on the roadmap instead of shipping half-right.

**Soroban versus classic operations.** A Soroban transaction cannot contain a classic Stellar operation, so settlement and the path payment to the supplier cannot be atomic yet. The front end chains two transactions, and we kept the atomic swap through a Soroban DEX on the roadmap.

**Testnet realities.** The mock oracle does not publish on its own and the contract refuses prices older than 900 s, so the live site started failing with "stale price" errors. We wrote a keepalive script that refreshes all four currencies every 10 minutes, a settlement keeper, and a one-command crash script for the video. Liquidity on the testnet DEX is our own offer, so we reset it between takes.

**Working as three in under 48 hours.** We split the contract into three modules with explicit interfaces: Hugo on the vault and front end, Lancelot on pricing, Clément on hedges and settlement. Each person worked in their own git worktree and branch with CI (cargo test plus the front-end build) on every push. Integration was done with real cross-module tests instead of mocks. Before submission we ran a coherence pass across code, docs, scripts and demo figures, so that every number in the README, the deck and the video (55,012.81 / 56,098.72) matches what the contract returns.

---

## What's next

**Short term (after the hackathon)**
- **Atomic settle-and-pay:** settle the hedge and pay the supplier in a single transaction through a Soroban DEX.
- **Productive margin:** the business's margin earns tokenized T-bill yield while the hedge is open.
- **Robust fixing:** TWAP over a window, sanity bounds, multiple oracles, and an admin fallback that settles at zero P&L if the oracle has no price at maturity.
- **Hardening:** an entry delay for LPs before a winning settlement, and checks on tranche ratios.

**Pool and risk**
- **Netting** per currency and per maturity date: importers and exporters offset each other, so only net exposure consumes capital. The exact basket computation is already written and tested.
- **Automatic hedging of the net exposure:** hold the currency in tokenized government bonds (Etherfuse CETES, EURC), with one position per currency and a rebalancing threshold, so the spread pays costs instead of risk.
- **Treasury management:** 8% USDC in the contract, the rest in tokenized T-bills, which pays the senior tranche from real yield.
- **Pricing controls:** price skew to attract the missing side, a dynamic spread, interest rates read on-chain, and fixings based on anchor SEP-38 quotes.

**Product and distribution**
- Direct delivery in local currency to the supplier from the pool's hedge position, saving two conversions.
- Transferable hedged invoices for factoring.
- A "Lock the rate" SDK for wallets and anchors.
- KYC through anchors (SEP-12).
- An audit, then mainnet. Reflector FX on mainnet already covers 24 currencies.

---

## Tech stack (≤ 25 words)

**Rust, Soroban (soroban-sdk), Stellar CLI, Reflector FX (SEP-40), USDC SAC, path payments, Etherfuse rates, React, TypeScript, Vite, stellar-sdk, Stellar Wallets Kit, Freighter, Vercel, Python.**

*(24 words)*

---

## AI tooling note

**Claude Code (Anthropic) was our main AI tool, used throughout the 48 hours.**

- **As a project manager across the team.** One Claude Code session held the full picture: the architecture, each teammate's module, the deadlines and the demo figures. It prepared each workstream (git worktree, branch, written brief with the expected interface), merged the branches, redeployed the testnet and kept the TODO current. That let three people work in parallel on vault, pricing and settlement without stepping on each other.
- **For implementation and tests.** It wrote and refactored Soroban code alongside us. For example it implemented `close_early` with a shared closing helper reused by `settle`, plus eight new tests. It also wrote the property tests for the capacity rule and the invariant checks. Every change went through `cargo test` (152 tests) before being merged.
- **For review and consistency.** We used multi-agent review passes: independent agents audited contract comments, error codes, scripts, front-end copy and demo numbers, then verified each other's findings. It flagged real issues, such as a settlement price read at call time instead of at maturity, and stale figures that would have contradicted the video.
- **For ops.** It wrote the testnet scripts (setup, seeding with real accounts, keeper, crash, oracle keepalive), diagnosed issues such as stale-oracle errors and a misaligned DEX offer, and prepared the Vercel deployment.
- **For product thinking.** An "LLM council" of five agents with different perspectives stress-tested the idea, the pitch and the MVP scope early on. It pushed us to cap the hedge (no margin calls) and to tie it to the supplier payment.

**Why it helped:** in 48 hours the bottleneck is not typing code. It is keeping a financial contract, a front end, scripts, docs and a video script consistent with each other while three people change them at the same time. AI let us spend our time on the product decisions (pricing model, risk rules, demo story) and verify more than we could have by hand. All financial logic and design decisions were reviewed and decided by the team.

*(Lancelot, Clément: add any other AI tools you used here.)*
