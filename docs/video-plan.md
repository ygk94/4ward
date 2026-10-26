# Vidéo 5 min : 2 min de slides, 3 min de démo

Deck : https://claude.ai/artifact/DkBw6TP7Xq6etfJv3h5nZo (voix off dans les notes de chaque slide).
Checklist technique avant chaque prise : `docs/video-script.md`, section « Avant d'enregistrer » (étapes a à g).

## Partie 1 : slides (0:00 → 2:00)

| Temps | Slide | Idée |
|---|---|---|
| 0:00 | cover | 4ward, la phrase d'accroche |
| 0:05 | problem | Lucía, 1 000 000 MXN dans 90 jours, +10 % = +5 500 $ |
| 0:25 | gap | banque, fintech, option, Stellar au comptant : rien pour elle |
| 0:45 | solution | bloquer le prix, déposer une garantie, régler et payer |
| 1:05 | numbers | peso +12 % : 61 600 sans protection, 56 099 avec, coût 0,6 % |
| 1:25 | holds | dépôt d'avance, règle de capacité, deux tranches |
| 1:40 | stellar | Soroban, Reflector, path payment, USDC, Etherfuse, Wallets Kit |
| 1:52 | demo | slide bleue de transition, on passe sur le site |

La slide « next » et la slide de clôture passent à la fin, après la démo.

## Partie 2 : démo (2:00 → 5:00)

Préparer trois fenêtres avant la prise : le site (4ward-fx.vercel.app, Freighter sur fx-lucia), un terminal à la racine du repo, l'éditeur avec les fichiers ci-dessous déjà ouverts aux bonnes lignes.

| Temps | À l'écran | Ce qu'on dit (en gros) |
|---|---|---|
| 2:00 → 2:15 | Page Vault : capital du pool, réservé, libre, Safe / Dynamic | Les LPs déposent des USDC et choisissent leur risque. |
| 2:15 → 2:40 | Écran de couverture : 1 000 000 MXN, 90 days, devis 55 012,81, protection 10 %, signature « Lock my price » | Le prix vient de l'oracle et de l'écart de taux. Une signature. |
| 2:40 → 3:00 | **Code** `contracts/pool/src/pricing.rs:204` `forward_rate` (num / den, le spread arrondi côté pool), puis `contracts/pool/src/vault.rs:587` `reserve` | Le prix à terme en entiers, et le pool refuse toute couverture au-delà de sa capacité. |
| 3:00 → 3:15 | Terminal : `./scripts/crash.sh 12`, puis Mes positions avec le compte à rebours | Le peso prend 12 %. Mode démo : 90 jours en 5 minutes. |
| 3:15 → 3:40 | **Code** `contracts/pool/src/pricing.rs:175` `fixing` puis `contracts/pool/src/hedge.rs:249` `pnl` (`clamp(-margin, margin)`) | Le règlement prend le cours à l'échéance, pas au moment du clic. Le gain est plafonné à la garantie. |
| 3:40 → 4:00 | Settle sur Mes positions (+11 002,56 USDC), puis « Pay my supplier » sur la même carte : aperçu du coût sur le DEX, signature, récapitulatif du coût total (56 098,72 USDC) | Le pool paie, puis le fournisseur reçoit 1 000 000 MXN pile, depuis l'app. |
| 4:00 → 4:15 | stellar.expert : la transaction et l'événement du règlement, le paiement reçu | Tout est vérifiable on-chain. |
| 4:15 → 4:35 | **Code** `contracts/pool/src/hedge.rs:189` `close_early`, puis le bouton « Close early » sur une autre couverture ouverte (aperçu du montant) | La PME peut sortir avant l'échéance, au prix du marché, le spread sert de frais de sortie. |
| 4:35 → 4:50 | Terminal : `cargo test` qui défile, « 152 passed » | 152 tests, dont l'invariant de solvabilité vérifié après chaque opération. |
| 4:50 → 5:00 | Slides « next » puis clôture | La suite, puis « 4ward. Lock today's price for tomorrow's invoice. » |

## Conseils de tournage

- Pour le code, zoomer l'éditeur (Cmd +) pour que 15 à 20 lignes remplissent l'écran, et surligner les lignes clés pendant qu'on parle.
- `cargo test` prend du temps : le lancer avant la prise et filmer seulement la fin, ou couper au montage.
- Ouvrir une seconde couverture avant la prise pour la séquence « Close early », afin qu'elle soit encore ouverte au moment de filmer.
- L'attente de l'échéance (5 min réelles) se coupe au montage. Les plans de code peuvent être tournés pendant ce temps.
