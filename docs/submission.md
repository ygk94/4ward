# 4ward, submission form

Live demo: https://4ward-fx.vercel.app
Repo: https://github.com/ygk94/4ward
Pool contract: https://stellar.expert/explorer/testnet/contract/CACSMEBQLBR73CH6BRPGY7LDDE2P63C6WPZEJPZNIP5DLT5IN6O5VWA4


## Tagline (112 characters)

Lock today's exchange rate for an invoice you pay in 90 days. No bank, no credit line: just a wallet on Stellar.


## Description

4ward lets a small business lock today's price for a foreign-currency invoice it will pay in the future, then pay its supplier in that currency on the due date, all on Stellar.

Lucía imports tequila into Lisbon. She owes her supplier in Guadalajara 1,000,000 Mexican pesos in 90 days. If the peso rises 10% by then, her invoice costs her about $5,500 more and her margin is gone. A large company would call its bank and buy a currency forward. Lucía has no credit line, and her bank will never offer her that product. Stellar already lets her pay abroad in seconds, but only at today's rate. Nothing lets her fix a rate for a payment she will make later.

With 4ward, Lucía enters her invoice and immediately sees a locked price: she will pay 55,013 USDC as long as the peso moves less than the protection she picks. This is the fair 90-day forward rate, meaning today's rate adjusted for the interest-rate gap between the dollar and the peso, plus a 0.5% spread. She picks a protection level, for example ±10%, deposits the matching margin of 5,501 USDC and signs once with her wallet.

On the due date anyone can trigger settlement. The contract reads the rate in force at maturity from a SEP-40 oracle (Reflector FX in production), not the rate at the moment someone clicks, so nobody can game the timing. If the peso went up, the pool pays her the difference. If it went down, the difference comes out of her margin. A Stellar strict-receive path payment then converts her USDC into pesos, and the supplier receives exactly 1,000,000 MXN. Within her protection, her total cost equals the price she locked on day one, up to the gap between the oracle fixing and the price she gets on the DEX (zero in our demo, where we provide the DEX liquidity ourselves). Beyond it, she only bears the part of the move past her limit.

In our demo the peso jumps 12% before maturity. Without 4ward, Lucía would pay 61,600 USDC. With 4ward she pays 56,099 USDC in total, because at settlement the pool pays her 5,501.28 USDC on top of returning her 5,501.28 deposit (11,002.56 credited).

The margin she chooses is both her maximum loss and her maximum protection. Everything that can be lost is already deposited in the contract when the hedge opens, so there are no margin calls, no liquidations, no credit risk and no server watching prices around the clock. Financially it is a collar centred on the forward rate. Because protection is capped in both directions, it costs about 0.6% of the invoice, roughly four times less than an option and about the price of a bank forward, without needing a bank. Plans change, so Lucía can also close early: the contract prices her exit at the market forward for the remaining time, the losing side pays, and the exit fee is the spread.

On the other side, liquidity providers deposit USDC into a Soroban vault and act as the counterparty, earning the spreads. The vault issues shares with two prices. Entering uses the full price, while leaving uses an exit price that excludes the capital reserved for open hedges, so nobody can walk away and leave potential losses to the others. A capacity rule caps the pool’s FX risk: the maximum losses of all open hedges can never exceed the capital of the Dynamic tranche, and the contract refuses any hedge that would break it. LPs choose their risk through two tranches in the same vault. The Safe tranche gets a 4.5% target rate paid first, funded by the Dynamic tranche until idle capital is invested, so it is not guaranteed. The Dynamic tranche takes the first loss and keeps the upside. The pool also tracks exposure per currency, with limits set by the admin, and our tests check after every operation that the contract's USDC balance equals LP capital plus the margins of open hedges.

Everything described here runs on Stellar testnet today. A single Soroban contract written in Rust handles quotes, opening, settlement at the maturity fixing, early close, the two-tranche vault and per-currency exposure, and emits events for deposits, withdrawals, hedge opening, settlement, early close and admin limit changes. Four currencies are open for hedging (MXN, BRL, ARS, EUR), each with its own minimum protection. The supplier payment runs in pesos on testnet, with test USDC and MXN assets issued by the team and our own DEX offer; the other three currencies need a DEX market or an anchor before suppliers can be paid in them. Interest rates are a snapshot of Etherfuse stablebond yields (USTRY at 3.6%, CETES at 5.55%, TESOURO, EUROB) pushed by the admin; ARS uses a hardcoded rate. The contract only talks to the standard SEP-40 oracle interface and was checked against Reflector FX on testnet. For the demo we plug in a mock oracle with the same interface so we can crash the peso live, and in production Reflector's address is simply passed to the constructor.

The React front end, deployed on Vercel, covers the quote and hedge screen, a positions page with settle and close early, the LP vault, and a Risk lab that runs a Monte-Carlo simulation of the pool in a web worker. Operator scripts set up the whole testnet in one command, seed it with six businesses and five LPs across both tranches, run a settlement keeper, crash the peso for the video and keep the oracle fresh. The pool contract has 152 tests, including property tests (155 with the mock oracle).

We chose Stellar because the hedge and the payment in the supplier's currency run on the same rail. USDC is native, Reflector publishes FX prices on-chain, path payments convert to pesos in one operation, and fees are low enough that hedging a few thousand dollars makes sense, which no bank offers today.

Some things were left out on purpose. Netting, where opposite clients offset each other so only the net exposure uses capital, is already computed exactly and property-tested in the contract but not yet switched on. Hedging the pool by holding the currency in tokenized government bonds, earning yield on idle treasury, and an atomic settle-and-pay are next. The MVP shows the safety net: capped hedges, a strict capacity rule and settlement at the maturity fixing.


## Problem solved

Small and medium businesses that trade across borders in emerging-market currencies face this problem every day. Think of importers paying suppliers in Mexico, Brazil or Argentina, exporters invoicing in dollars while their costs are local, or European companies paying in dollars. Their invoices are typically due in 30 to 120 days. A 6 to 10% currency move over 90 days is common for the Mexican peso and routine for the Brazilian real or the Argentine peso. For a business running 5 to 10% margins, one move can wipe out the profit on a whole order.

Large companies handle this with bank forwards. Small ones are mostly locked out. Banks want a credit line and minimum sizes, FX fintechs ask for KYC, an upfront deposit and only cover a short list of currencies, and options cost around 2.6% of the invoice for 90 days. So most small businesses stay unhedged and simply absorb the risk. Crypto rails made cross-border payments fast and cheap, but only at today's rate.

4ward solves this with a capped, fully collateralized FX forward on Soroban, tied to the payment itself. The business deposits a margin of 5 to 20% of the invoice (higher minimums for volatile currencies: 7% BRL, 15% ARS), which is both its maximum loss and its maximum protection, so there is no credit to underwrite and no bank involved. The price is fair and transparent: the forward rate follows interest-rate parity using tokenized bond yields from Etherfuse, plus a visible 0.5% spread, for a total cost of about 0.6% of the invoice. Settlement uses the oracle rate at maturity and anyone can trigger it. The supplier is then paid in their own currency through a strict-receive path payment, so within its protection the business pays the price it locked, up to the gap between the oracle fixing and the DEX execution price. If plans change, it can close early at the market price. The other side of every hedge is an open liquidity pool whose solvency is enforced by the contract itself rather than by trust.


## Challenges

Demoing a 90-day product in three minutes. A forward is priced on its duration, so a 60-second hedge would show a price close to spot. We added an admin-only time factor: the price uses the announced 90 days while the real maturity falls five minutes later, so the video shows the true price and settles it live.

Settling at the right price. Our first settle read the oracle when called, so whoever settled could pick their moment. We moved to the fixing at maturity through SEP-40 price-at-timestamp, which meant adding price history to our mock oracle so it behaves like Reflector.

Keeping the pool solvent by construction. Two share prices, a capacity rule, rounding in favour of the pool and an invariant test after every operation. Our property tests also showed that naive netting underestimates losses, so we kept it switched off.

Platform limits. Soroban cannot mix in classic operations, so settle and the supplier payment are two transactions. On testnet the mock oracle never publishes alone, so we wrote a keepalive, a keeper and a crash script.

Teamwork. Three people, under 48 hours, three modules with clear interfaces, one branch and worktree each, CI on every push.


## What's next

In the short term we want to make settlement and the supplier payment a single atomic transaction through a Soroban DEX, let the business's margin earn tokenized T-bill yield while the hedge is open, and make the fixing more robust with a TWAP, sanity bounds, several oracles and an admin fallback for the rare case where no price exists at maturity. We would also add an entry delay for LPs before a winning settlement and stricter checks on the tranche ratio.

On the pool side, the big step is netting per currency and maturity, so that importers and exporters offset each other and only the net exposure consumes capital. The exact computation already exists and is tested. After that comes automatic hedging of the net exposure, holding the currency in tokenized government bonds such as Etherfuse CETES or EURC with one position per currency and a rebalancing threshold, so the spread pays for costs rather than risk. The idle treasury would sit mostly in tokenized T-bills, which would pay the Safe tranche from real yield. Price skew, a dynamic spread and interest rates read on-chain would complete the picture.

On the product side, we want to deliver local currency to the supplier straight from the pool's hedge position, make hedged invoices transferable for factoring, offer a simple "lock the rate" SDK to wallets and anchors, and plug KYC through anchors with SEP-12. Then an audit and mainnet, where Reflector FX already covers 24 currencies.


## Tech stack (18 words)

Rust, Soroban, Reflector FX, SEP-40, USDC, path payments, Etherfuse, React, TypeScript, Vite, stellar-sdk, Stellar Wallets Kit, Freighter, Vercel


## AI tooling note

We used Claude Code from Anthropic throughout the hackathon.

One Claude Code session acted as a project manager for the team. It held the full picture of the architecture, each person's module, the deadlines and the demo figures. It prepared each workstream with its own git worktree, branch and written brief, merged the branches, redeployed the testnet and kept the task list up to date. That is what allowed three people to build the vault, the pricing and the settlement in parallel without stepping on each other.

It also wrote and refactored code with us. For example, it implemented early close with a closing routine shared with settle, along with eight new tests, and it wrote the property tests for the capacity rule and the solvency invariant. Nothing was merged without the full test suite passing. We used multi-agent review passes, where independent agents audited contract comments, error codes, scripts, front-end copy and demo numbers and then checked each other's findings. They flagged real issues, such as settlement reading the price at call time instead of at maturity, and outdated figures that would have contradicted the video. Claude Code also wrote our testnet scripts, diagnosed problems like stale oracle prices and a misaligned DEX offer, and prepared the Vercel deployment. Early on, a council of five agents with different viewpoints stress-tested the idea and the MVP scope, which pushed us toward a capped hedge with no margin calls tied directly to the supplier payment.

In 48 hours the bottleneck is not typing code. It is keeping a financial contract, a front end, scripts, documentation and a video script consistent while three people change them at the same time. AI let us spend our time on the decisions that matter, like the pricing model, the risk rules and the demo story, and verify far more than we could have by hand. Every financial rule and design choice was reviewed and decided by the team.
