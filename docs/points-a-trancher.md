# Points à trancher

État au 25/10 soir. Le contrat est complet dans `main` et le testnet est vérifié. **Les trois points sont réglés** (mergés dans `main` et validés sur le testnet le 25/10 soir). Le point 4 liste les limites connues du contrat, documentées et à corriger après la deadline.

---

## 1. ✅ Cours de règlement : fixing à l'échéance (réglé : `settle` règle au cours en vigueur à l'échéance, erreur 312 si absent ; historique dans le mock-oracle)

**Qui :** Clément (côté `settle`) + Lancelot (`fixing(currency, maturity)` et historique du mock-oracle). Pris en charge le 25/10 soir.

**Le problème.** `settle` lit le cours de l'oracle **au moment où on l'appelle**, pas à l'échéance. Comme n'importe qui peut appeler `settle` une fois l'échéance passée, celui qui règle choisit son moment : la PME attend que le peso aille dans son sens, ou le pool l'inverse. C'est une option gratuite, que le pool n'a pas facturée.

**Le correctif.**
- Pricing : une lecture `fixing(currency, maturity)` qui utilise `price(asset, timestamp)` de Reflector (le cours en vigueur à l'échéance).
- Mock-oracle : garder un historique des prix (`set_price` horodaté, `price(asset, ts)` renvoie le prix en vigueur à cet instant).
- Paiement : `settle` utilise ce fixing au lieu de `spot`.

**Bonus.** Ça règle aussi le problème de fraîcheur du prix au moment du `settle` pendant la démo, et c'est la première condition pour brancher le netting côté vault.

**Si on ne le fait pas :** le dire dans la présentation comme limite connue du MVP.

---

## 2. ✅ Durée de la démo vs prix affiché (réglé : `set_time_factor 25920`, posé par le script de setup ; prix à 90 jours, échéance réelle 5 min ; scénario de la vidéo validé au centime : 56 098,72 USDC (taux Etherfuse) ; 56 112,69 avec les anciens taux 400/700)

**Qui :** Hugo (vidéo, front) + Clément + Lancelot

**Le problème.** La vidéo raconte une facture à **90 jours** : prix figé **54 873 USDC** (à l'époque, taux 400/700 ; 55 012,81 avec les taux Etherfuse). Mais pour régler en direct, on ouvre la couverture avec une échéance de **60 s**, et le prix figé devient ≈ cours du jour + spread : **55 275 USDC**. Le chiffre à l'écran ne correspond plus à l'histoire.

**Options.**

| Option | Ce qu'on fait | Pour | Contre |
|---|---|---|---|
| a. Assumer | Afficher « échéance de démo : 60 s » et raconter avec le prix réel de 60 s | Rien à coder, honnête | L'écart forward / spot (le cœur du pricing) n'apparaît pas dans la démo |
| b. Facteur d'accélération | Paramètre admin (testnet seulement) : le prix est calculé sur la durée annoncée (90 j), l'échéance réelle est divisée par un facteur (90 j → 60 s) | La démo montre le vrai prix à 90 j et se règle en direct | À coder dans `hedge.rs` (calcul de `maturity`) + un réglage admin ; à présenter comme un mode démo |
| c. Deux couvertures | Une à 90 j pour montrer le prix, une à 60 s pour montrer le règlement | Rien à coder | Plus long et plus confus à l'écran |

**Recommandation :** b si Clément a 30 min, sinon a.

**Décision (25/10 soir) : option b, prise en charge par Clément.** Conséquence : les chiffres de la vidéo repasseront à ceux de 90 jours (prix figé 55 012,81, avec protection 56 098,72, taux Etherfuse ; 54 873,05 / 56 112,69 avec les anciens taux 400/700). Le script actuel (`docs/video-script.md`) utilise ceux de 60 s, à mettre à jour une fois le facteur en place.

---

## 3. ✅ Codes d'erreur du Pricing (réglé le 25/10 : PricingError 300-312, mergé)

**Qui :** Lancelot

**Le problème.** Les refus du Pricing sont des `panic!("currency not enabled")`, `"stale price"`, `"rate not set"`, `"cap below minimum"`… Les textes sont effacés dans le contrat compilé : le front reçoit une erreur générique et ne peut pas afficher « protection minimale pour le peso : 5 % ».

**Le correctif.** `#[contracterror] enum PricingError` avec des codes 300-399, comme le vault (100-112) et le paiement (200-205).

---

## 4. ⬜ Limites connues du contrat (documentées, correctifs après la deadline)

Aucune n'est corrigée dans le contrat avant la soumission ; elles sont résumées dans le README (*Limites connues du contrat*).

**a. Fixing indisponible (302/312) = couverture irréglable.** `settle` lit le cours en vigueur à l'échéance (`pricing::fixing`). S'il n'y a aucun prix de moins de 900 s avant l'échéance (302), ou si l'échéance dépasse l'historique de l'oracle (312 : 64 prix sur le mock, ~12 h sur Reflector), l'erreur est **définitive** : un prix posé après l'échéance ne compte jamais, aucune fonction ne passe `settled` à true ni ne libère `Reserved`. La marge de la PME et le capital réservé restent bloqués. *Correctif :* fonction admin `settle_fallback(id)` qui, après l'échéance + un délai de grâce, fait `close_position` et rend la marge avec un P&L nul. *Pour la vidéo :* 90 j uniquement, `crash.sh` ou `crash.sh --refresh` pendant les 5 min avant l'échéance, keeper actif.

**b. Mode démo (`time_factor` > 1).** Le prix porte sur la durée annoncée et l'échéance est raccourcie, donc le carry n'est pas neutralisé : avec un oracle immobile, on encaisse l'écart forward / spot sans risque en quelques minutes. `time_factor` doit valoir 1 en production (à terme : `set_time_factor` derrière une feature cargo).

**c. Dépôt juste avant un `settle` gagnant.** Le fixing est public dès l'échéance, alors que `settle` est appelé plus tard : un LP peut déposer au prix plein juste avant un règlement gagnant pour le pool, puis retirer juste après. *Correctif :* délai d'entrée, ou blocage de `deposit_lp` tant qu'une couverture échue n'est pas réglée.

**d. Versement poussé dans `settle`.** Si le transfert USDC au propriétaire échoue (trustline supprimée, compte désautorisé par l'émetteur), tout le `settle` échoue et le capital réservé reste bloqué tant que le propriétaire ne corrige pas. *Correctif :* modèle pull (solde réclamable + `claim()`).

**e. Tranches : intérêts senior et `Reserved`.** Les intérêts senior (4,5 %/an, temps réel) sont pris sur le junior : la valeur junior peut passer sous `Reserved` (capacité négative) ou sous 20 % sans aucune opération, et `withdraw_senior` ne vérifie pas que le capital restant couvre `Reserved` (un `settle` perdant pour le pool échouerait alors en 103). *Correctif :* dans `withdraw_senior`, refuser si `total_deposits - amount < reserved` (nouveau code 113).

**f. Dernière part junior.** Si toutes les parts junior sont brûlées alors qu'il reste du capital (`Reserved`), le déposant suivant en hérite (et des pertes éventuelles). *Correctif :* refuser de brûler la dernière part tant que `reserved > 0`.

**g. Débordements dans `to_usdc` / `mul_div_*`.** Un notionnel démesuré fait trapper la VM au lieu de renvoyer un code (aucun risque de fonds, `vault::notional_usdc` est protégé). *Correctif :* `checked_mul` et `PricingError::Overflow = 313`.

---

## Chiffres de référence (pour la vidéo et le front)

Taux Etherfuse figés dans `scripts/rates.snapshot` (USD 3,60 % USTRY, MXN 5,55 % CETES, 25/10 22h47 UTC). Facture 1 000 000 MXN, spot 0,055, spread 0,5 %, plafond 10 %, 90 jours (mode démo : échéance réelle 5 min). Vérifié sur le testnet le 25/10 au centime.

| | Montant |
|---|---|
| Prix figé | 0,0550128 → **55 012,81 USDC** |
| Dépôt (marge) | 5 501,28 USDC |
| Crash +12 % (spot 0,0616), versé à Lucía au règlement | 11 002,56 USDC |
| Coût total sans protection | 61 600 USDC (+6 587) |
| **Coût total avec protection** | **56 098,72 USDC** (+1 086) |

Ne pas relancer `scripts/sync-rates.sh --snapshot` avant l'enregistrement : les chiffres changeraient.
