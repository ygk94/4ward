# 4ward: Friction Log (HackMeridian 2026)

Team: Hugo Thomas, Clément Darmon, Lancelot Watelle. Built in Lisbon, 25-26 October 2026, in under 48 hours.
Live demo (testnet): https://4ward-fx.vercel.app

Time costs below are rough estimates reconstructed after the event, not measured.

## What we built

4ward lets a small business lock today's exchange rate for a foreign-currency invoice it will pay later (for example 1,000,000 MXN in 90 days). On the due date it pays its supplier in that currency on Stellar.

- **`contracts/pool`** (Soroban, Rust): a USDC vault with two LP tranches (Safe / Dynamic). It handles the forward pricing (spot plus the interest-rate gap, using Etherfuse yields), capped hedges with a margin deposit, `quote` / `open_hedge` / `settle` / `close_early`, a capacity rule and a solvency invariant. The contract has 152 host tests.
- **`contracts/mock-oracle`**: a SEP-40 oracle with the same read interface as Reflector FX (`lastprice`, `price(asset, timestamp)`, 14 decimals), including price history.
- **Front end** (React, TypeScript, Vite): quote, hedge, positions, vault and risk pages. After settlement it pays the supplier with a USDC → MXN path payment.
- **Scripts**: testnet setup and seeding, an oracle keepalive, a settlement keeper, a "peso crash" simulator, Etherfuse rate sync and DEX offer alignment.

## Stellar tools we used

| Tool | How we used it |
|---|---|
| **Soroban smart contracts / `soroban-sdk` 28** | Pool and mock-oracle contracts: `require_auth`, events, instance and persistent storage with `extend_ttl`, cross-contract calls to the USDC SAC and to the oracle. |
| **Stellar CLI** | `stellar contract build`, `deploy`, `invoke` for every admin and demo step, `keys` for the test identities, `contract bindings typescript`, and `tx new manage-sell-offer` / `path-payment-strict-receive` to validate the payment flow before wiring it into the UI. |
| **`@stellar/stellar-sdk` 17.4 (JS)** | Soroban RPC (simulate, send, poll), reading contract events for the activity feed, and building the classic path payment. |
| **Stellar Wallets Kit 2.7.1 + Freighter** | Wallet connection and transaction signing in the front end. |
| **Horizon (testnet)** | Reading and adjusting DEX offers, and estimating the path payment cost. |
| **Reflector FX (SEP-40)** | Target oracle. We checked our interface against Reflector on testnet and copied its MXN rate into our mock (`keepalive.sh --live`). |
| **USDC as a SAC, test MXN asset, path payment strict receive** | Margins and payouts in USDC. The supplier receives the exact peso amount. |
| **stellar.expert** | Checking contract state, events and transactions during development and the demo. |
| **Raven MCP / other Stellar MCPs** | Not used. |

## Friction points

### 1. A Soroban transaction cannot contain classic operations
- **What happened:** we wanted settlement and the supplier payment (a classic path payment) to happen in one atomic transaction. That is not possible.
- **Cost:** about 2 h (design rethink, two-step UI, explaining it in the pitch).
- **Workaround:** two transactions and two wallet signatures (`settle()`, then the path payment). An atomic version through a Soroban DEX (e.g. Soroswap) is on our roadmap.

### 2. No live FX oracle feed we could control on testnet
- **What happened:** our contract rejects prices older than 900 s, and settlement needs a price published close to maturity. Our mock oracle does not publish on its own, so hedges failed with errors #302/#312 (stale or missing price). Two seeded hedges got stuck as unsettleable.
- **Cost:** about 4-5 h across the weekend.
- **Workaround:** `keepalive.sh` (refreshes prices every 10 min), `keeper.sh` (settles at maturity) and `crash.sh` (sets demo prices). We documented the limitation.

### 3. Settling at the price "at maturity"
- **What happened:** our first `settle` read the spot price at call time, so the caller could choose the moment. The fix was to use SEP-40 `price(asset, timestamp)`, but there are few examples of the fixing pattern, and our mock needed price history to behave like Reflector.
- **Cost:** about 3 h.
- **Workaround:** price history in the mock oracle and a `fixing()` lookup within 900 s before maturity.

### 4. Demonstrating a 90-day product in a 3-minute video
- **What happened:** with no way to fast-forward ledger time on testnet, a real short hedge would be priced near spot and look wrong.
- **Cost:** about 2 h.
- **Workaround:** an admin-only `time_factor`. The price uses the announced 90 days, while the real maturity falls 5 minutes later.

### 5. Opaque contract and wallet errors in the front end
- **What happened:** failures came back as raw codes (`Error(Contract, #302)`, token errors #10 / #13) or as `[object Object]` when the user closed the wallet modal or refused to sign.
- **Cost:** about 2-3 h.
- **Workaround:** a hand-written mapping from error codes to messages in the front end, including the SAC token error codes we had to find by trial and error.

### 6. Redeploys change contract IDs
- **What happened:** every contract change meant redeploying and copying new IDs into `.env.testnet`, `front/.env.example`, the README and the docs. Each teammate also needed their own environment.
- **Cost:** about 2 h overall.
- **Workaround:** a single `setup-testnet.sh` that writes all IDs, plus CLI aliases (`fx-pool`, `fx-mock-oracle`).

### 7. Keeping the DEX in line with the oracle
- **What happened:** after a simulated crash, the test MXN DEX offer no longer matched the oracle price, so the path payment cost was wrong or the payment failed (`op_underfunded`, no path).
- **Cost:** about 1.5 h.
- **Workaround:** `dex-align.sh` reads offers from Horizon and re-places them at the oracle price.

### 8. Toolchain setup
- **What happened:** contracts need the `wasm32v1-none` target and Rust ≥ 1.84 (older Rust cannot build contracts), plus a recent stellar-cli. Not every teammate had this ready.
- **Cost:** about 1 h total.
- **Workaround:** pinned versions and setup steps in `AGENTS.md` / `TODO.md`.

### 9. Storage TTL
- **What happened:** archival and TTL are easy to forget. We had to choose thresholds and extend the instance, shares and exposures ourselves.
- **Cost:** about 1 h.
- **Workaround:** `extend_ttl` on every write (about 120 days).

## What worked well

- Host-side unit tests with `soroban-sdk` testutils are fast and expressive. Property tests on the solvency invariant caught real issues.
- Stellar CLI covers the whole lifecycle (build, deploy, invoke, classic ops) and made the flow easy to script and to demo from a terminal.
- Path payment strict receive is exactly right for paying a supplier an exact foreign amount.
- The USDC SAC behaves like any token from Soroban.
- Stellar Wallets Kit made Freighter integration quick.
- stellar.expert was very useful for debugging events and state.

## What would have made it easier

1. A way to combine a Soroban call with classic operations atomically, or an official Soroban DEX swap example covering classic assets.
2. A live, regularly updated FX oracle on testnet (Reflector FX with history), so teams do not have to run their own price pusher.
3. A documented example of the SEP-40 "fixing at timestamp" pattern for settlement.
4. Human-readable contract errors in the SDK, wallets and CLI (spec-based error names), and a published list of SAC error codes.
5. Testnet time controls or a documented pattern for demoing time-based products.
6. A first-party helper for managing contract IDs and environments across redeploys and teammates.
