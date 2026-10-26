# Runbook vidéo démo : exécution par Claude

Pour : Claude (Claude Code), qui enchaîne la démo sur le testnet, filme, monte et livre `demo-video/out/4ward-demo.mp4`.
Source du récit : `docs/video-script.md`. Ce runbook le reprend avec trois changements : une partie « investissement » (Safe / Dynamic), la phrase sur Etherfuse corrigée (roadmap, pas en place) et « can't go bankrupt » retiré.
Commandes du parcours : `docs/payment-cli.md`. Voix off en anglais, consignes en français, zéro jargon à l'écran (règle du council).

Consigne pour Claude : suivre les phases dans l'ordre et s'arrêter à chaque **STOP** pour prévenir l'utilisateur. Ne jamais lancer `setup-testnet.sh`, ne jamais redéployer, ne pas lancer `crash.sh --reset`.

---

## 0. Paramètres

| Paramètre | Valeur par défaut | Variante |
|---|---|---|
| Facture | 1 000 000 MXN, Import (`BuyCurrency`), 90 jours (`7776000` s) | |
| Protection | **±10 %** (`cap_bps 1000`) : montre le plafond | **±15 %** (`cap_bps 1500`) : Lucía paie exactement le prix bloqué |
| Crash | +12 % (0,055 → 0,0616) | |
| Spot de départ | 0,055 (`5500000000000`) | |
| Signature | l'utilisateur clique « Approve » dans Freighter quand Claude le demande | signataire de démo local (voir § 8, point 1) |
| Voix off | `say -v Daniel` (TTS macOS) | voix enregistrée par un humain (`demo-video/vo/*.m4a`), recommandé |
| Front | local : `pnpm dev` dans `front/` (contient Risk lab, pas encore commité) | Vercel, une fois le front poussé |

Chiffres (vérifiés le 26/10 par une simulation de `quote` sur le pool déployé : forward 0,0550128, prix bloqué 55 012,81) :

| Chiffre | ±10 % | ±15 % |
|---|---|---|
| Prix bloqué | 55 012,81 | 55 012,81 |
| Dépôt (deposit) | 5 501,28 | 8 251,92 |
| Sans protection (1 M × 0,0616) | 61 600 (+6 587) | 61 600 (+6 587) |
| Protection versée par le pool | 5 501,28 | 6 587,19 |
| Crédité au règlement (dépôt + protection) | 11 002,56 | 14 839,11 |
| Payé au total avec 4ward | 56 098,72 (+1 086) | **55 012,81 (+0)** |

La voix off ci-dessous est écrite pour ±10 %. Pour ±15 %, remplacer les phrases marquées **[±15]**.

---

## 1. Pré-vol (Claude, environ 10 min)

Tout en lecture, sauf indication contraire. Chaque point doit être vert avant la phase 3.

```sh
cd /Users/famillethomas/Projet/fistea-hackmeridian && source .env.testnet
stellar contract invoke --id fx-pool --network testnet --source-account fx-admin --send=no -- time_factor        # 25920
stellar contract invoke --id fx-pool --network testnet --source-account fx-admin --send=no -- tranche_values     # junior > 0, senior > 0
stellar contract invoke --id fx-pool --network testnet --source-account fx-admin --send=no -- capacity           # > dépôt
stellar contract invoke --id $USDC_SAC --source-account fx-lp --network testnet --send=no -- balance --id $(stellar keys address fx-lucia)   # > 62 000 USDC
curl -s "https://horizon-testnet.stellar.org/offers?seller=$ISSUER" | grep -o '"id": "[0-9]*"'                 # offre 931579 vivante
./scripts/keeper.sh --dry-run          # couvertures échues en attente ? si oui : ./scripts/keeper.sh --once (écriture)
pgrep -fl keepalive.sh                 # tourne sans --keeper : OK (il conserve le prix du crash)
```

Outils :
- `ffmpeg` (`brew install ffmpeg`, absent au 26/10).
- Autorisation **Enregistrement de l'écran** pour l'application qui fait tourner Claude Code (Réglages → Confidentialité et sécurité). Test : `screencapture -v -V 3 /tmp/t.mov`.
- Claude in Chrome connecté (`tabs_context_mcp`).
- Freighter sur **Testnet**, compte `fx-lucia` importé et connecté au front.
- Front : `cd front && pnpm dev`, puis ouvrir `http://localhost:5173` et vérifier `#/`, `#/vault`, `#/positions` et `#/risk`.

Mise en scène : fenêtre Chrome en 1920×1080, zoom 110 %, un seul onglet, Ne pas déranger activé, Terminal.app en police 20 pt, fond sombre.

**STOP** : faire le point avec l'utilisateur (paramètres du § 0, signature, voix).

---

## 2. Organisation des fichiers

```
demo-video/                 # à ajouter dans .gitignore
  cards/*.html              # cartes et incrustations (générées par Claude)
  cards/*.png               # rendu : Chrome headless --screenshot --window-size=1920,1080
  raw/NN-nom.mov            # rushes (screencapture -v)
  vo/NN.aiff|m4a            # voix off par plan
  out/4ward-demo.mp4        # livrable 1080p, H.264, AAC, ≤ 3:00
  timings.txt               # horodatage réel de chaque étape (preuve : txs, échéance)
```

Méthode de capture : `screencapture -v -V <secondes> -R <x,y,w,h> raw/NN.mov`, lancé en arrière-plan, pendant que Claude pilote Chrome (navigate, clics, saisie) ou Terminal.app (`osascript -e 'tell app "Terminal" to do script "…"'`). Prévoir une seconde de marge avant et après chaque action. Couper au montage.

---

## 3. Cartes et incrustations (à préparer avant la prise)

HTML statique dans le style du front (fond `#fbf8f1`, police Geist, bleu `#2f6bff`), rendu en PNG 1920×1080 avec fond transparent pour les incrustations.

| Fichier | Contenu |
|---|---|
| `c01-title` | `4ward` + « Lock today's price for tomorrow's invoice » |
| `c02-invoice` | Facture : *Tequila Jalisco → Lucía, Lisbon — 1,000,000 MXN — due in 90 days* |
| `c03-peso10` | Courbe simple MXN +10 % → « +5,500 USD » en rouge |
| `c04-bank` | « Bank FX hedge: credit line required » barré |
| `o05-peso12` | Incrustation « Peso +12% » |
| `o06-compare` | Côte à côte : *Without protection: 61,600 USDC (+6,587)* / *With 4ward: 56,099 USDC (+1,086)* **[±15 : 55,013 USDC (+0)]** |
| `o07-demomode` | « Demo mode: 90 days run in 5 minutes » |
| `c08-recap` | Prix bloqué 55,013 · payé 56,099 · sans protection 61,600 **[±15 : payé 55,013]** |
| `c09-stack` | Wordmarks texte : Stellar · USDC · Reflector · Etherfuse · Stellar Wallets Kit (pas de logos officiels sauf fichiers fournis) |
| `c10-next` | Next : the pool hedges itself in Etherfuse tokenized bonds · idle capital earns treasury yield · importers matched with exporters · « lock the rate » button for every Stellar wallet |
| `c11-end` | `4ward` + phrase d'accroche |

---

## 4. Prise de la démo on-chain : séquence chronométrée

Fenêtre critique : l'échéance réelle tombe **300 s après `open_hedge`**. Le crash (C3) doit partir **avant** cette échéance. Le `settle` (C5) part environ 10 s **après** l'échéance, et peu après (historique du mock : 64 prix).

| # | Quand | Qui | Action | Capture |
|---|---|---|---|---|
| P1 | T−3 min | Claude, terminal | `./scripts/crash.sh --demo` (MXN à 0,055, moins de 900 s avant l'ouverture) + offre DEX à 0,055 (`payment-cli.md` § 5, `--offer-id 931579 --price 550000:10000000`) | non |
| C1 | T−1 min | Claude, Chrome | `#/` → HedgeScreen : saisir 1,000,000, MXN, Import, chip **90 days**, adresse fournisseur (préremplie), protection ±10 % **[±15]** | `raw/10-form.mov` |
| C1b | | | Vérifier à l'écran **55,012.81 USDC** et le dépôt **5,501.28** **[8,251.92]**. Sinon : arrêter et recalculer (formules de `video-script.md`) | dans 10 |
| C2 | **T0** | Claude clique « Lock my price » ; **l'utilisateur approuve dans Freighter** | `open_hedge`. Noter l'heure et le hash (lien « View on explorer » de `TxStatus`) dans `timings.txt` | `raw/11-sign.mov` (fenêtre Freighter visible) |
| C2b | T+20 s | Claude, Chrome | Ouvrir le lien stellar.expert de la tx, montrer l'événement `hedge_opened` | `raw/12-expert-open.mov` (5 s) |
| C3 | T+60 s | Claude, Terminal.app | `./scripts/crash.sh 12` (0,055 → 0,0616) | `raw/20-crash.mov` |
| C3b | T+90 s | Claude, terminal | Offre DEX à 0,0616 (`--price 616000:10000000 --offer-id 931579`) | non |
| C4 | T+2 min → T+5 min | Claude, Chrome | `#/positions` : compte à rebours de la couverture. Filmer les 15 dernières secondes et le passage à « Due · settle now » | `raw/30-countdown.mov` |
| C5 | T+5 min 10 s | Claude clique « Settle » ; **l'utilisateur approuve** | `settle` → message « Settled: 11,002.56 USDC paid out » **[14,839.11]** | `raw/31-settle.mov` |
| C6 | T+6 min | Claude, Terminal.app | Path payment (`payment-cli.md` § 6, `--send-max 619100000000`) ; signé par `fx-lucia` dans le CLI, sans Freighter | `raw/40-pathpay.mov` |
| C6b | | Claude, Chrome | stellar.expert : la tx de path payment, zoom sur **1,000,000 MXN** reçus par le fournisseur | `raw/41-expert-pay.mov` |

Hors chrono (avant ou après) :

| # | Action | Capture |
|---|---|---|
| C7 | `#/vault` : les deux cartes Safe / Dynamic, puis survol de la barre de répartition et du badge « Solvent » | `raw/50-vault.mov` (20 s) |
| C8 | `#/risk` : le Monte-Carlo qui tourne, l'histogramme de Dynamic, la ligne Safe 4,5 % | `raw/51-risk.mov` (15 s) |
| C9 (option) | Clôture anticipée sur une couverture ouverte en plus : bouton « Close early », aperçu du montant récupéré, sans confirmer | `raw/60-close.mov` (6 s) |

Si C3 tombe après l'échéance, si `settle` renvoie une erreur 312, ou si un écran plante : refaire la prise depuis P1. Pas besoin de recommencer les autres plans. Le parcours CLI complet (`payment-cli.md`) sert de secours. Entre deux prises, `crash.sh --demo` remet le prix à 0,055 (sinon le crash de +12 % se cumule).

**STOP** : montrer à l'utilisateur 3 images extraites de chaque rush (`ffmpeg -ss … -frames:v 1`) avant le montage.

---

## 5. Montage : plans, durées et voix off (3:00)

### 1. Le problème : 0:00 → 0:25
| Plan | Voix off |
|---|---|
| c01 → c02 | *This is Lucía. She imports tequila into Lisbon, and she owes her supplier in Guadalajara one million pesos, due in 90 days.* |
| c03 | *If the peso rises 10% before then, that invoice costs her 5,500 dollars more.* |
| c04 | *A large company would ask its bank to lock the rate. Lucía has no credit line. On Stellar she can pay abroad in seconds, but only at today's rate. Until now.* |

### 2. Bloquer le prix : 0:25 → 1:00
| Plan | Voix off |
|---|---|
| raw/10 | *She enters her invoice: one million pesos, 90 days, her supplier's address. The app reads the peso rate from a Reflector-compatible price feed and locks a price for 90 days: 55,013 USDC, whatever happens.* |
| raw/10 (slider) | *She picks her protection, plus or minus 10%, and deposits 10% as a guarantee. That deposit is the most she can ever lose. No credit check.* **[±15 : plus or minus 15%, and deposits 15%]** |
| raw/11 → raw/12 | *One signature, and the price is locked, on Stellar, for anyone to check.* |

### 3. Le peso flambe : 1:00 → 1:15
| Plan | Voix off |
|---|---|
| raw/20 + o05 | *Now the peso jumps 12%.* |
| o06 | *Without protection, her invoice costs 6,587 dollars more. With 4ward, she only pays the part beyond her 10% limit: about 1,086 dollars.* **[±15 : With 4ward, she pays exactly the price she locked.]** |

### 4. Échéance et paiement : 1:15 → 1:55
| Plan | Voix off |
|---|---|
| raw/30 + o07 (coupe pendant l'attente) | *On the due date, anyone can trigger settlement, at the rate of that day, so nobody can game the timing.* |
| raw/31 | *The contract pays out her protection: 11,002 USDC back in her wallet.* **[±15 : 14,839 USDC]** |
| raw/40 → raw/41 | *A Stellar path payment converts her dollars and pays her supplier exactly one million pesos, in seconds.* |
| c08 | *Two transactions today; one click is on our roadmap.* |

### 5. L'autre côté : un nouvel investissement : 1:55 → 2:35
| Plan | Voix off |
|---|---|
| raw/50 (cartes) | *Who stands behind Lucía? Anyone holding dollars. 4ward is also a new kind of investment: earning from small businesses protecting their trade.* |
| raw/50 (Safe) | *Safe earns a fixed yield, paid first. It only loses if Dynamic is wiped out.* |
| raw/50 (Dynamic) → raw/51 | *Dynamic takes the currency risk and earns the rest: a fee on every protection sold. Our risk lab simulates it, year by year.* |
| raw/50 (badge Solvent) | *And the pool never promises more than it holds: every protection is reserved upfront.* |

### 6. Stellar et la suite : 2:35 → 3:00
| Plan | Voix off |
|---|---|
| c09 | *USDC, Reflector prices, and Etherfuse's tokenized treasury rates built into every locked price.* |
| c10 | *Next: the pool protects itself by holding pesos in Mexican government bonds tokenized by Etherfuse, idle dollars earn treasury yield for Safe investors, and every Stellar wallet gets a "lock the rate" button.* |
| c11 | *4ward. Lock today's price for tomorrow's invoice.* |

Règles de montage :
- Voix off d'abord : générer chaque plan, mesurer sa durée (`ffprobe`), puis caler les rushes dessus en coupant ou en accélérant les temps morts (×2 au plus, jamais sur un chiffre).
- Les chiffres doivent rester lisibles au moins 2 s.
- Fondus de 0,3 s entre les parties. Musique : aucune par défaut.
- Rendu : `ffmpeg` concat (filtre `concat` + `overlay` pour les PNG transparents + `adelay` / `amix` pour la voix), 1920×1080, 30 i/s, `-c:v libx264 -crf 20 -c:a aac`.

---

## 6. Vérifications avant livraison

- Durée ≤ 3:00 (`ffprobe`).
- Extraire une image par plan et les relire : chiffres conformes au § 0, aucune clé ou adresse privée à l'écran, aucune notification.
- Les hashes de `timings.txt` (open, settle, path payment) existent sur stellar.expert.
- Réécouter les phrases : rien ne présente la couverture du pool par Etherfuse comme déjà en place.
- Livrer à l'utilisateur : chemin du mp4, durée, liste des txs. L'upload (YouTube non répertorié, Loom…) est fait par l'utilisateur.

---

## 7. Ce que Claude ne peut pas faire seul

1. **Signer dans Freighter** : les fenêtres d'extension sont hors de portée de Claude in Chrome. L'utilisateur doit être devant l'écran pour C2 et C5 (deux clics « Approve »). Sans lui : signataire de démo (§ 8, point 1) ou parcours entièrement en CLI.
2. **Une voix naturelle** : `say` donne une voix synthétique correcte mais robotique. Une voix humaine enregistrée sur le texte du § 5 est nettement meilleure ; Claude cale ensuite le montage dessus.
3. **Les permissions macOS** (Enregistrement de l'écran) et `brew install ffmpeg` : à accorder ou valider par l'utilisateur.

## 8. Décisions à prendre avant l'exécution

1. Signature : l'utilisateur présent (recommandé, le plan Freighter reste dans la vidéo), ou un signataire de démo local dans le front (`VITE_DEMO_SECRET`, `.env.local` seulement, jamais sur Vercel ; demande une modification de `wallet.ts`).
2. Protection : ±10 % (montre le plafond) ou ±15 % (prix tenu au centime près).
3. Voix : TTS ou voix humaine.
4. Front : local (`pnpm dev`) ou Vercel (commiter et pousser d'abord Risk lab, `PoolPanel`, `entry.ts`).
5. Logos officiels (Stellar, USDC, Reflector, Etherfuse, SWK) : fournir les fichiers, sinon wordmarks en texte.
