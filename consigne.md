# Consigne : suite de 4ward après HackMeridian

Passation rédigée le 11/11/2026, pour reprendre le travail sur une autre machine. À lire en entier avant de commencer.

## 1. Où on en est

- HackMeridian est fini : **4ward a terminé 3e**. Pitch live le mercredi 28/10.
- **57 Blockchain** nous accompagne pour la suite. Nos contacts : **Leo** et **Jia Chen**. Leur outil s'appelle **Compass** : c'est lui qui a analysé notre repo (au commit `ac2013c`) et produit 4 rapports et un prototype.
- Les rapports **ne sont pas dans le repo** (il est public). Hugo les transfère à part : les ranger dans `docs/57-blockchain/` et ajouter `docs/57-blockchain/` à `.git/info/exclude` pour ne jamais les committer. S'ils manquent, les demander à Hugo avant de commencer.
- Contenu :
  - `01-deck-review.pdf` : passer du deck pour un jury au deck pour des investisseurs ;
  - `02-investor-qa.pdf` : les questions qu'un investisseur posera, avec pour chacune ce qu'on peut déjà répondre et ce qui manque ;
  - `03-next-phase-plan.pdf` : le chemin du testnet à un pilote mainnet, en 4 jalons ;
  - `04-feedback.pdf` : 5 questions ouvertes, idées produit, notes sur le prototype ;
  - `prototype.html` : maquette sur données fictives (positions, transparence LP, échéanciers, roll, rappels).

## 2. Ce qu'on a promis à 57 Blockchain

1. **Un one-pager**, à envoyer cette semaine. Leo s'en servira pour nous présenter aux équipes et institutions de l'écosystème Stellar.
2. **Les premiers correctifs du contrat** (phase 1 de leur plan). Une fois faits, on envoie le commit ou la branche à Jia : iels relancent Compass sur la nouvelle version et donnent leur avis sur les changements du contrat avant l'audit.

Côté Stellar : on n'a encore parlé à personne au SCF (Stellar Community Fund). On va contacter **Gildas Soeiro** par un ami qui le connaît.

## 3. Notre cible, et un point à régler

**Cible annoncée** (celle du pitch du 28/10, et celle écrite à Leo) : les **producteurs mexicains qui exportent aux États-Unis**. Ils facturent en dollars mais paient leurs coûts en pesos, donc un peso qui monte réduit leur marge. Ils sont trop petits pour un forward bancaire ou pour les minimums des fintechs FX. On veut les atteindre via les anchors et wallets Stellar qui gèrent déjà des flux USD/MXN.

**Deux incohérences à traiter :**

- **Même côté du book.** Notre README (section *Pool hedging*, point 1, *Netting*) le dit lui-même : un exportateur mexicain payé en dollars craint que le peso monte, **comme Lucía**, l'importatrice de la démo qui doit payer en pesos. Ils sont du même côté. Avec une cible faite seulement d'exportateurs, on a le book à sens unique dont parlent les rapports : environ 4 % de spread nécessaires (`sim/out/calibration.md`) contre 0,5 % facturés. La contrepartie naturelle est un importateur mexicain qui paie ses fournisseurs en dollars, ou une entreprise payée en pesos.
- **Le produit montré ne correspond pas à la cible.** La démo, le front et le parcours de paiement servent une PME qui doit **payer** des pesos à un fournisseur. Un exportateur a besoin de bloquer la valeur en pesos de **créances en dollars**. Le contrat sait-il déjà traiter ce sens ? À vérifier dans `contracts/pool` avant de promettre quoi que ce soit.

À trancher avec l'équipe avant d'écrire le one-pager.

## 4. L'essentiel des rapports

**Deck (01)**
- À garder : le slogan « Pay later. At today's price. », l'exemple de Lucía, la comparaison 0,59 % contre 2,2 à 2,6 %, la slide Why Stellar.
- À corriger :
  - la slide marché confond volume couvert et revenu. 210 M$ de volume donnent environ 1,05 M$ de spread, qui va aux LP, et environ 210 k$ pour le protocole à 20 %. Ces 20 % n'existent que dans la simulation : le contrat verse tout le spread au pool ;
  - « a pool that can't go bust » est plus que ce que le MVP garantit. Formulation proposée : « the pool only accepts hedges it can pay in full » ;
  - « 4 → any » devient « 4 devises sur testnet, 24 sur Reflector mainnet, chacune ouverte quand elle est couvrable ».
- À ajouter : traction, business model, économie du pool et risque, go-to-market, la demande de financement. Ordre proposé en 13 slides dans le rapport.
- La slide la plus importante : l'économie du pool. Le risque est dans le déséquilibre ; voici comment on le gère (netting, skew de prix, couverture en CETES) et à partir de quel volume ça marche.

**Q&A investisseurs (02)**
- Plus gros manque : **la preuve de demande** (entretiens, lettre d'intention, partenaire pilote, discussion avec un anchor).
- Autres manques : bottom-up du marché, qui met le premier argent dans le pool, réponse sur la réglementation (un forward vendu à des PME est probablement réglementé : MiFID II en UE, peut-être au Mexique ; à vérifier avec un avocat), plan et budget d'audit (programme Audit Bank de la SDF), une fintech concurrente dont on connaît les prix MXN, une ligne par membre de l'équipe.
- Trois réponses à savoir par cœur : qui porte le risque, comment on gagne de l'argent, pourquoi les PME viendront.

**Plan (03)**
| Jalon | Contenu | Effort estimé par eux |
|---|---|---|
| M1 : pilote mainnet audité | Correctifs 4a à 4g (voir `docs/points-a-trancher.md` §4), plafond des intérêts Safe, multisig, pause, upgrade, limites pilote (whitelist, plafond par trade et total, 120 j max), paramètre *worst-rate* sur `open_hedge`, audit externe | 2 à 3 semaines-ingé + 4 à 8 semaines d'audit |
| M2 : paiement en une signature | `settle` crédite un solde réclamable ; `pay_supplier(hedge_id, max_usdc)` fait le swap sur un DEX Soroban. À vérifier d'abord : liquidité MXN sur les DEX Soroban mainnet | 2 à 4 semaines |
| M3 : book dans les deux sens | Fixings hebdomadaires, règlement par panier, capacité par `basket_max_loss`, skew de spread entre 0,25 et 1 % | 3 à 5 semaines |
| M4 : pool productif | USDC, T-bills tokenisés et CETES, couverture du net, Safe à taux variable | 4 à 6 semaines |

**Feedback (04) : les 5 questions qu'ils nous posent**
1. Que veut-on prouver d'abord : la sécurité sur mainnet, ou l'économie du pool ?
2. Qui couvre l'écart entre 0,5 et 4 % : des LP, une contrepartie institutionnelle, ou des flux dans les deux sens ?
3. 4ward est-il un outil de couverture, ou un moyen de payer une facture à prix bloqué ?
4. Dans quelle devise nos clients tiennent-ils leurs comptes ? (Lucía est à Lisbonne mais bloque un coût en USDC.)
5. Garde-t-on un prix oracle (jusqu'à 15 min d'âge), ou passe-t-on à des cotations de market makers ?

Le plan précise que les réponses à ces questions changent l'ordre des phases.

## 5. Ce qu'il y a à faire

1. **Trancher** la cible (section 3) et les 5 questions (section 4).
2. **One-pager** pour Leo : problème, cible, produit, pourquoi Stellar, état actuel (testnet, tests, 3e à HackMeridian), prochaines étapes, ce qu'on cherche (intros anchors et wallets USD/MXN, SCF). En anglais.
3. **Phase 1 du contrat** : correctifs 4a à 4g et les ajouts du plan 03. C'est ce qu'on enverra à Jia.
4. **Deck investisseur** selon le rapport 01.
5. **Préparation du Q&A** selon le rapport 02.
6. **Plus tard** : phases 2 à 4, prototype (`docs/57-blockchain/prototype.html`) comme source d'idées pour le front.

## 6. Règles à respecter

- Lire `AGENTS.md` : build avec `stellar contract build`, tests avec `cargo test`.
- L'interface de `contracts/pool/src/lib.rs` est figée : le front s'appuie dessus. Tout changement de signature (`open_hedge` avec *worst-rate*, `claim`, `settle_fallback`, etc.) doit être signalé et répercuté dans `front/`.
- Codes d'erreur : Vault 100-199, Paiement 200-299, Pricing 300-399. Ne jamais réutiliser un numéro.
- Conventions : prix en USD par unité de devise à 14 décimales, montants à 7 décimales, taux en bps, tout en `i128`, arrondis en faveur du pool.
- Ne jamais committer son propre `.env.testnet`.
- **Le repo `ygk94/4ward` est public** : rien de confidentiel dedans, et demander avant de pousser.
