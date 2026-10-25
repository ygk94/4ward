# Points à trancher

État au 25/10 soir. Le contrat est complet dans `main` et le testnet est vérifié. Points 1 et 2 pris en charge par Clément (avec Lancelot pour le fixing), point 3 réglé.

---

## 1. 🔴 Cours de règlement : fixing à l'échéance — EN COURS

**Qui :** Clément (côté `settle`) + Lancelot (`fixing(currency, maturity)` et historique du mock-oracle). Pris en charge le 25/10 soir.

**Le problème.** `settle` lit le cours de l'oracle **au moment où on l'appelle**, pas à l'échéance. Comme n'importe qui peut appeler `settle` une fois l'échéance passée, celui qui règle choisit son moment : la PME attend que le peso aille dans son sens, ou le pool l'inverse. C'est une option gratuite, que le pool n'a pas facturée.

**Le correctif.**
- Pricing : une lecture `fixing(currency, maturity)` qui utilise `price(asset, timestamp)` de Reflector (le cours en vigueur à l'échéance).
- Mock-oracle : garder un historique des prix (`set_price` horodaté, `price(asset, ts)` renvoie le prix en vigueur à cet instant).
- Paiement : `settle` utilise ce fixing au lieu de `spot`.

**Bonus.** Ça règle aussi le problème de fraîcheur du prix au moment du `settle` pendant la démo, et c'est la première condition pour brancher le netting côté vault.

**Si on ne le fait pas :** le dire dans la présentation comme limite connue du MVP.

---

## 2. 🔴 Durée de la démo vs prix affiché

**Qui :** Hugo (vidéo, front) + Clément + Lancelot

**Le problème.** La vidéo raconte une facture à **90 jours** : prix figé **54 873 USDC**. Mais pour régler en direct, on ouvre la couverture avec une échéance de **60 s**, et le prix figé devient ≈ cours du jour + spread : **55 275 USDC**. Le chiffre à l'écran ne correspond plus à l'histoire.

**Options.**

| Option | Ce qu'on fait | Pour | Contre |
|---|---|---|---|
| a. Assumer | Afficher « échéance de démo : 60 s » et raconter avec le prix réel de 60 s | Rien à coder, honnête | L'écart forward / spot (le cœur du pricing) n'apparaît pas dans la démo |
| b. Facteur d'accélération | Paramètre admin (testnet seulement) : le prix est calculé sur la durée annoncée (90 j), l'échéance réelle est divisée par un facteur (90 j → 60 s) | La démo montre le vrai prix à 90 j et se règle en direct | À coder dans `hedge.rs` (calcul de `maturity`) + un réglage admin ; à présenter comme un mode démo |
| c. Deux couvertures | Une à 90 j pour montrer le prix, une à 60 s pour montrer le règlement | Rien à coder | Plus long et plus confus à l'écran |

**Recommandation :** b si Clément a 30 min, sinon a.

**Décision (25/10 soir) : option b, prise en charge par Clément.** Conséquence : les chiffres de la vidéo repasseront à ceux de 90 jours (prix figé 54 873,05, avec protection 56 112,69). Le script actuel (`docs/video-script.md`) utilise ceux de 60 s, à mettre à jour une fois le facteur en place.

---

## 3. ✅ Codes d'erreur du Pricing (réglé le 25/10 : PricingError 300-311, mergé)

**Qui :** Lancelot

**Le problème.** Les refus du Pricing sont des `panic!("currency not enabled")`, `"stale price"`, `"rate not set"`, `"cap below minimum"`… Les textes sont effacés dans le contrat compilé : le front reçoit une erreur générique et ne peut pas afficher « protection minimale pour le peso : 5 % ».

**Le correctif.** `#[contracterror] enum PricingError` avec des codes 300-399, comme le vault (100-110) et le paiement (200-204).

---

## Chiffres de référence (pour la vidéo et le front)

Facture 1 000 000 MXN, spot 0,055, taux USD 4 % / MXN 7 %, spread 0,5 %, plafond 10 % :

| | 90 jours | 60 secondes |
|---|---|---|
| Prix figé | 0,054873 → **54 873,05 USDC** | 0,055275 → **55 275 USDC** |
| Marge déposée | 5 487,31 USDC | 5 527,50 USDC |
| Crash +12 % (spot 0,0616), sans protection | 61 600 (+6 727) | 61 600 (+6 325) |
| Crash +12 %, avec protection | 56 112,69 (+1 240) | 56 072,50 (+798) |
