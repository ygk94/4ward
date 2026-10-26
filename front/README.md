# Front

Vite + React + TypeScript, `@stellar/stellar-sdk`, `@creit.tech/stellar-wallets-kit` (Freighter et autres wallets).

```sh
pnpm install
pnpm dev
```

Les IDs de contrats sont lus dans `.env.local` (variables `VITE_*`), généré par `../scripts/setup-testnet.sh`.
Écrans : accueil et présentation (`home/`), couverture (devis `quote` + `open_hedge`, `payment/`), vault à deux tranches (`vault/`), mes positions (`settle`, `positions/`).
