# Council — HackMeridian FX hedge (25/10/2026)

## Question cadrée
QUESTION AU COUNCIL — Hackathon HackMeridian (Stellar, Lisbonne), 25/10/2026

L'équipe (étudiants ingénieurs ESILV spécialisés finance de marché ; lead dev à l'aise en Rust/Solana, débutant Soroban) s'engage sur ce projet et veut l'approfondir AVANT de coder : quelles faiblesses, quel angle de pitch, quel périmètre MVP réaliste, quelle différenciation, quelles priorités techniques ?

LE PROJET (nom à trouver) : « Bloquez aujourd'hui le taux de change d'un paiement futur, dans n'importe quelle devise, sur Stellar. »
- Problème : Stellar règle très bien les paiements internationaux au comptant (path payments, anchors, stablecoins), mais les factures d'entreprise se paient à 30/60/90 jours ; la devise peut bouger de 5 à 15 % entre-temps. Les forwards de change bancaires sont chers, opaques, réservés aux grosses boîtes. Rien de natif sur Stellar.
- Produit : NDF (Non-Deliverable Forward) réglé en USDC. La PME saisit « je dois payer 1 000 000 MXN dans 90 jours », reçoit un taux forward F = Spot × (1 + r_devise·t)/(1 + r_USD·t) ± spread, dépose une marge (≥5 %) en USDC. À l'échéance, settle() permissionless lit l'oracle et règle la différence en USDC. Le paiement réel passe ensuite par path payment / anchor (hors protocole).
- Forward PLAFONNÉ : la marge = perte max ET protection max (5 % de marge ⇒ protégé à ±5 %). Zéro risque de crédit, pas d'appel de marge ni de liquidation.
- Contrepartie : un pool USDC de LPs (parts SEP-41), qui gagnent le spread, les pertes des utilisateurs, et le rendement Blend sur le capital non réservé.
- Netting par devise (jambes contre USD) et par échéance ; règle de capacité : perte max = Σ(exposition nette × plafond) ≤ dépôts ⇒ le pool ne peut pas faire faillite. Limites par devise, skew de prix, plafond min plus élevé sur devises instables.
- Oracle Reflector (SEP-40) pour le FX (couverture des devises à vérifier ; mock sinon). Paires croisées via cours contre USD.
- Stack : Soroban (Rust), Reflector, Blend, USDC, front + wallet (Freighter ?) à définir.
- MVP prévu : pool USDC unique, 2–3 devises (MXN, EUR, + une émergente), échéances 30/60/90 j, taux d'intérêt en dur, netting + capacité, Blend, front PME + LP, démo avec mouvement de change simulé et règlement.
- Concurrence connue à vérifier : « Hedgehog FX Swap » (déjà soumis dans l'écosystème Stellar).

CONTEXTE ÉQUIPE / LEÇONS PASSÉES :
- Hackathon précédent (XRPL) : 5e place ; jury : idée pertinente mais manque d'ORIGINALITÉ et de « peps », et la brique « analyse » n'apportait rien de visible en démo. Règle de l'équipe : privilégier l'originalité et un cas d'usage incarné ; la vitrine fait les points.
- Leçon récente : valider tôt « pourquoi une blockchain ? » ; un projet précédent ce même hackathon a été abandonné car il se réduisait à « stocker un hash ».
- Durée de build limitée (hackathon), démo live devant jury.

ENJEU : choisir le bon périmètre et le bon angle maintenant ; un mauvais choix = MVP trop gros non démontrable ou pitch « encore un protocole DeFi » qui ne se distingue pas.

## Réponses des conseillers (anonymisées, résumées)
**Response A:**
Verdict : faisable, mais le MVP actuel est trop gros pour un dev débutant en Soroban. Il faut couper la moitié.
À virer tout de suite : Blend (intégration cross-contract lourde, auth délicate, invisible en démo → slide roadmap). Le netting par échéance et le skew de prix : on garde une seule règle de capacité globale Σ(notionnel × plafond) ≤ dépôts. Les parts LP SEP-41 : un simple mapping deposits[addr]. La 3e devise : EUR et MXN seulement.
Ce qui reste, un seul contrat : deposit_lp, open_forward(devise, notionnel, échéance), settle(id), withdraw. Taux forward calculé dans le contrat avec taux en dur, en arithmétique entière i128.
Point bloquant pour la démo : le temps. Oracle mock admin-settable derrière une interface compatible SEP-40, échéance en secondes pour la démo (90 « jours » = 90 secondes). Reflector branché en lecture pour afficher le spot réel.
Première heure : stellar contract init, Freighter testnet, USDC testnet ou token SAC créé soi-même ; vérifier les paires FX Reflector testnet (si pas MXN → mock sans débat) ; front en parallèle (Next.js + stellar-sdk + Freighter API) avec données factices.
Démo (3 min) : « Tacos Lisboa » doit 1 M MXN, bloque son taux, dépose 5 %. Le peso chute de 8 % via le mock, settle : l'USDC arrive dans le wallet, plafond visible. Un écran, une jauge avant/après.
Tests : écrire settle en premier, 3 tests (hausse, baisse, au-delà du plafond).

**Response B:**
La faille centrale : le plafond annule l'argument de vente. Vous dites que la devise peut bouger de 5 à 15 %, puis vous protégez à ±5 %. Pour couvrir 15 %, la PME doit immobiliser 15 % du nominal en USDC pendant 90 jours ; une banque fait un forward sur ligne de crédit sans bloquer un centime. Le produit coûte plus cher en trésorerie que celui qu'il remplace.
Le netting suppose des flux dans les deux sens, et ils ne viendront pas : les PME sont des importateurs qui paient en MXN, toutes dans le même sens. Le pool porte un pari directionnel nu sur le peso. Qui est le LP rationnel ?
Prix et règlement fragiles : taux en dur = prix forward inventé ; Reflector ne couvre probablement pas le MXN ; un NDF réglé sur un seul relevé d'oracle est manipulable ; un NDF vendu à des PME est un dérivé réglementé (EMIR, MiFID).
La démo répète l'erreur XRPL : un forward à 90 jours ne se voit pas ; le « mouvement de change simulé » est un curseur. Avec Hedgehog FX Swap déjà présent : pitch « encore un protocole DeFi ».
Périmètre irréaliste pour un débutant Soroban : coupez Blend et le netting multi-échéances.
Question évitée : pourquoi une PME passerait de l'euro à l'USDC puis au MXN via un anchor pour couvrir un risque qu'elle aurait pu facturer en euros ?

**Response C:**
Le vrai produit n'est pas « un NDF sur Stellar ». C'est la facture qui porte sa propre couverture.
1. Couverture attachée à la facture : le fournisseur émet une facture on-chain, la couverture y est collée dès l'émission ; montant USD figé pendant que le peso s'effondre en live ; histoire incarnée (Lucía, importatrice de tequila) ; la couverture se règle toute seule au règlement de la facture.
2. Position transférable = financement de créances : si la position est un token SEP-41, la PME peut vendre sa facture couverte à un financeur (affacturage), slide roadmap.
3. Le pool comme carnet de couverture de tout Stellar : carte du monde où les expositions opposées s'annulent.
4. Anchors et MoneyGram comme canal de distribution : SDK « Bloquer le taux » pour n'importe quel wallet.
5. Démo « crash button » : le jury choisit la devise et fait plonger l'oracle de 12 %, règlement en direct, pool solvable.
Noms : Pinned, Locktide. Hedgehog : eux swap pour traders, vous facture pour PME.

**Response D:**
Trop de jargon (NDF, netting, skew, SEP-41, Blend…) : un juré non trader décroche. Le pitch en une phrase : « María importe des pièces du Mexique, doit 1 M de pesos dans 3 mois ; si le peso monte, sa facture coûte 50 000 $ de plus et sa marge disparaît. » La formule n'a rien à faire sur une slide.
Le forward plafonné est la meilleure idée mais présenté à l'envers : c'est une assurance taux de change, prime connue d'avance, remboursement plafonné.
« Le paiement réel passe hors protocole » = vous ne payez pas la facture ; alors pourquoi Stellar ? Le produit doit faire le paiement.
Les LPs sont un second public invisible : un seul personnage, une seule histoire en démo.
Démo : curseur « le peso bouge de +8 % » et deux comptes côte à côte, sans protection −40 000 $, avec −5 000 $ remboursés automatiquement.
Test : expliquer en 20 s à quelqu'un hors finance ; chaque mot qui suscite une question sort du pitch.

**Response E:**
Vous ne construisez pas le bon produit. Le NDF existe parce que les banques ne peuvent pas livrer certaines devises ; sur Stellar, livrer est natif (path payments, anchors). Un NDF réglé en USDC jette ce que Stellar sait faire.
Ce que la PME veut : que sa facture coûte X USDC aujourd'hui et que le fournisseur reçoive exactement 1 000 000 MXN dans 90 jours. Le vrai produit : une facture à taux figé, un escrow programmable qui, à l'échéance, déclenche lui-même le path payment vers le fournisseur. Couverture et paiement dans la même transaction.
Le pool de LPs : la contrepartie naturelle de l'importateur US est l'exportateur mexicain ; deux PME qui se couvrent l'une l'autre, le pool comme filet. C'est le netting en une phrase.
Le plafond de 5 % : avec des mouvements de 5 à 15 %, la PME reste exposée au-delà ; assumer un tunnel de change (range forward) avec un prix clair.
Périmètre : un seul corridor USD→MXN, une seule facture, un moment fort : le peso chute de 12 %, le fournisseur reçoit quand même ses 1 000 000 MXN pile. Couper Blend, la 3e devise, les échéances multiples.

## Mapping, revues croisées et faits vérifiés
MAPPING : A=Executor, B=Contrarian, C=Expansionist, D=Outsider, E=First Principles.

PEER REVIEWS (résumé) :
- Les 5 relecteurs désignent B (Contrarian) comme réponse la plus forte, E (First Principles) juste derrière.
- Angles morts : A (Executor) ne questionne ni le produit ni la différenciation ; C (Expansionist) empile des features sans périmètre ni faisabilité.
- Manqués par tous :
  * Un forward plafonné symétrique est en fait un collar / combinaison d'options (call spread long + put spread short) ; il faut le dire et justifier le pricing (symétrique autour de F ⇒ valeur ≈ 0 hors skew, donc spread acceptable) ; taux MXN en dur = risque d'arbitrage.
  * Anti-sélection : les PME se couvrent quand elles anticipent un mouvement.
  * Qui est le LP / amorçage de liquidité ; en démo l'équipe dépose elle-même.
  * Soroban : TTL/rent des entrées de stockage pour des positions de 90 jours ; échéance basée sur ledger.timestamp vs démo accélérée.
  * Critères du jury / tracks / temps de build restant.
  * Variante sans NDF : swap anticipé sur le DEX Stellar.
  * Le « pourquoi une blockchain » doit être tranché en une phrase : atomicité couverture + paiement, ouverture sans banque.

FAITS VÉRIFIÉS (agent de recherche) :
- HackMeridian : hack du dim 25/10 09h00 au LUNDI 26/10 12h00, pas de soumission tardive. Finalistes pitchent mercredi 28/10 à Meridian. Jugement asynchrone sur vidéo démo + deck + repo GitHub + README clair obligatoire. Critères Genesis : clarté du problème, qualité du MVP, intégration Stellar, potentiel de continuation. Tracks Genesis : « Stellar Unlocked » (fonctions du protocole de base, ex. path payments) et « Stellar Composed » (≥2 outils de l'écosystème : Reflector, Blend, Freighter…), cumulables. Prix 30 k$ (6 gagnants, en XLM).
- Reflector FX testnet CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W : assets EUR, GBP, CHF, CAD, MXN, ARS, BRL, THB, XAU ; base USD, 14 décimales, résolution 300 s. Crate sep-40-oracle 1.4.0 (client + mock).
- Blend testnet pool TestnetV2 CCEBVDYM… utilise son propre USDC (CAQCFVLO…), pas l'USDC Circle ; interface submit(from, spender, to, Vec<Request>) avec Supply=0 / Withdraw=1 ; crate blend-contract-sdk.
- USDC Circle testnet SAC CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA, faucet Circle 20 USDC / 2 h / adresse.
- Concurrence : FX Swap by Hedgehog (SCF #21, 42,5 k$) = swap prêt/emprunt USDC/EURC/AUDD avec livraison physique, pour traders ; pas un NDF. Aucun forward/NDF pour PME trouvé.
- Toolchain : stellar CLI 28.1, soroban-sdk 28 (workspace), @stellar/stellar-sdk 17.4, @creit.tech/stellar-wallets-kit 2.7.1, @stellar/freighter-api 6.0.1.

## Verdict du chairman
## Où le council est d'accord

- **Le périmètre est trop gros pour ~18 h de build et un dev qui débute sur Soroban.** Les cinq coupent Blend ; quatre sur cinq coupent aussi le netting multi-échéances et la 3e devise.
- **Un forward à 90 jours ne se voit pas en démo.** Il faut un moment fort : faire chuter l'oracle en direct, montrer le règlement et l'USDC qui arrive.
- **Une seule persona PME, un seul corridor (USD→MXN), aucun jargon à l'écran.** Les LPs ne sont pas un personnage de la démo.
- **Le plafond est la bonne idée, mais mal vendu** : c'est une protection dont on connaît d'avance le coût maximum, pas un « NDF plafonné ».

## Où le council se divise

- **Le produit.** L'Executor construit le NDF tel quel, réduit. First Principles et Outsider veulent qu'il paie le fournisseur : un NDF réglé en USDC laisse de côté ce que Stellar fait le mieux, livrer la devise. L'Expansionist veut « la facture qui porte sa propre couverture ».
- **L'économie.** Le Contrarian juge la proposition de valeur cassée : protéger 15 % oblige à immobiliser 15 % en USDC, le netting ne trouvera pas de flux inverses, le LP porte un pari nu sur le peso. Les autres jugent ces problèmes réels mais solubles par le pitch et le paramétrage.
- **L'ambition.** L'Expansionist veut affacturage, carte mondiale des expositions et SDK pour anchors ; les autres coupent tout.

## Angles morts repérés en revue croisée

- Un forward plafonné symétrique est un **collar** : centré sur F, sa valeur est proche de zéro, donc seul le spread rémunère le pool. Des taux d'intérêt en dur créent un risque d'arbitrage, à présenter comme limite connue.
- **Anti-sélection** : une PME se couvre surtout quand elle s'attend à un mouvement.
- **Amorçage de la liquidité** : en démo, l'équipe alimente le pool.
- **Soroban** : TTL des positions à prolonger (`extend_ttl`) ; échéance sur `ledger.timestamp`, donc échéance courte en démo.
- **Le jury note sur vidéo, deck, repo et README**, sans démo live. Critères : clarté du problème, MVP, intégration Stellar, potentiel de suite. Deux tracks cumulables : Unlocked (path payments) et Composed (Reflector + Freighter suffisent).
- **Hedgehog** = swap avec livraison physique pour traders, pas une couverture pour PME.

## La recommandation

**Produit recadré :** une facture fournisseur à taux figé. La PME bloque aujourd'hui le coût en USDC d'un paiement en pesos à 90 jours ; à l'échéance, la couverture se règle toute seule et le fournisseur reçoit ses pesos par path payment.

**Pitch 20 s :** « Lucía importe de la tequila à Lisbonne. Elle doit 1 000 000 de pesos à son fournisseur dans 90 jours. Si le peso monte de 10 %, elle perd toute sa marge. Sa banque ne lui fera jamais de couverture de change. Avec [Nom], elle fige son prix en 30 secondes depuis son wallet, et le jour J son fournisseur est payé en pesos, au taux promis. »

**MVP IN :** un contrat (`deposit_lp`, `open_hedge`, `settle` permissionless, `withdraw`) ; USDC Circle testnet ; spot réel Reflector + oracle mock SEP-40 admin pour le bouton crash ; échéance en secondes ; une règle de capacité Σ(montant × plafond) ≤ dépôts ; front + Freighter, deux écrans ; path payment USDC→MXN de test vers le fournisseur ; README soigné.

**MVP OUT :** Blend, parts LP SEP-41, netting par devise/échéance, skew, affacturage, SDK anchors → slide « suite ».

**Décisions :** Blend non. Parts LP = mapping. Netting pitché, pas codé (la contrepartie naturelle de l'importateur est l'exportateur mexicain, le pool sert de filet). Contrat générique sur les actifs Reflector, démo sur MXN, UI liste MXN/BRL/ARS. Path payment vers le fournisseur : oui (actif MXN de test + offre DEX) ; une tx Soroban ne contient pas d'opération classique, donc le front enchaîne `settle()` puis le path payment, sans atomicité stricte : le dire honnêtement.

**Objection « plafond 5 % vs mouvements 5–15 % » :** le plafond est choisi par la PME (10 % de marge = ±10 %), minimum plus élevé sur devises instables. C'est un tunnel de change à perte max connue. Face à la banque : la PME n'a pas de ligne de crédit, l'alternative réelle est « aucune couverture » ; la marge immobilisée est le prix d'une couverture sans banque.

**Pourquoi Stellar :** « Sur Stellar, la couverture et le paiement en devise du fournisseur passent par le même rail : USDC natif, oracle FX Reflector et path payment jusqu'au peso. Pas de banque, pas de ligne de crédit, un règlement que n'importe qui peut déclencher. »

**Démo en 5 temps (vidéo 3 min) :**
1. Lucía et sa facture de 1 000 000 MXN à 90 jours, spot réel lu sur Reflector.
2. Plafond 10 %, taux figé, dépôt de marge, signature Freighter.
3. Bouton « Crash du peso » (+12 %) : sans [Nom] −120 000 $, avec −20 000 $.
4. Échéance : `settle()` crédite l'USDC, le path payment livre 1 000 000 MXN au fournisseur, visible sur l'explorateur.
5. Panneau pool solvable, capacité respectée ; slide « suite » : netting, affacturage, SDK anchors.

**Noms :** Pinned, Locktide, Fixa.

## La première chose à faire

Écrire `settle()` avec ses trois tests (hausse, baisse, au-delà du plafond) contre un oracle mock SEP-40, avant de toucher au front. En parallèle, un coéquipier crée l'actif MXN de test et l'offre sur le DEX testnet.

## Positions des conseillers

- **Contrarian** — le plafond contredit l'argument de vente, netting illusoire, LP sur pari nu. *attaque l'économie*
- **First Principles** — Stellar sait livrer la devise : la couverture doit payer le fournisseur. *recadre le produit*
- **Expansionist** — facture qui porte sa couverture, transférable, distribuée par les anchors. *élargit*
- **Outsider** — une persona, zéro jargon, le plafond vendu comme une assurance. *clarifie le pitch*
- **Executor** — un contrat, oracle mock, sans Blend ni SEP-41, `settle` d'abord. *coupe le périmètre*
