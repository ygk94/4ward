# Front

Vite + React + TypeScript, `@stellar/stellar-sdk`, `@creit.tech/stellar-wallets-kit` (Freighter et autres wallets).

```sh
pnpm install
pnpm dev
```

Les IDs de contrats sont lus dans `.env.local` (variables `VITE_*`), généré par `../scripts/setup-testnet.sh`.
Liste des variables et valeurs du testnet actuel : `.env.example` (copier en `.env.local` si le setup n'a pas tourné sur la machine). Le build échoue s'il en manque une.

## Déploiement (Vercel)

Site statique, pas de back : le front lit et écrit directement sur le testnet (RPC public, signature par le wallet de l'utilisateur).

- Import du repo sur vercel.com, **Root Directory = `front`** (le reste est dans `vercel.json`), puis Settings → Environment Variables : les clés de `.env.example`.
- Ou en CLI depuis `front/` : `vercel` puis `vercel --prod` (variables via `vercel env add NOM production`).
- Après un redéploiement du pool par `setup-testnet.sh`, mettre à jour `VITE_POOL_ID` (et les autres IDs qui ont changé) sur Vercel, puis redéployer.
Écrans : accueil et présentation (`home/`), couverture (devis `quote` + `open_hedge`, `payment/`), vault à deux tranches (`vault/`), mes positions (`settle`, `positions/`).
