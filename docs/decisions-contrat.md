# Décisions sur le contrat

Une entrée par décision technique prise par l'équipe : le contexte, ce qui a été choisi, ce que ça implique, et quand la revoir. Ajouter les nouvelles décisions à la suite, ne jamais réécrire une ancienne : la remplacer par une nouvelle qui la cite.

---

## D1. Pas de provision d'intérêts Safe dans la capacité (pour l'instant)

**Date :** 24/11/2026. **Décidé par :** Hugo. **Lot :** B5 (intérêts Safe plafonnés).

### Contexte

Les LP de la tranche Safe (senior) reçoivent un taux visé de 4,5 % par an. Tant que la trésorerie du pool n'est pas placée, ces intérêts sont pris sur la tranche Dynamic (junior). Avec le temps, ils peuvent faire passer la valeur junior sous le capital réservé aux couvertures ouvertes. Un LP Safe qui retire tout à ce moment-là emporte une partie du capital qui garantit les couvertures, et un règlement perdant pour le pool échoue ensuite (erreur 103). C'est le défaut que B5 corrige.

Deux mécanismes étaient prévus :

1. **Plafonner le retrait Safe** : un LP Safe ne peut retirer que sa part de « capital total − capital réservé ». Les intérêts qui dépassent restent dus et sont versés quand le junior se reconstitue.
2. **Provisionner les intérêts** : la capacité du pool (le capital disponible pour ouvrir des couvertures) retranche d'avance 120 jours d'intérêts Safe, soit environ 1,48 % de ce qui est dû aux LP Safe. Le plafond du point 1 ne jouerait alors presque jamais.

### Décision

On fait le point 1. **On ne fait pas le point 2 pour l'instant.**

### Pourquoi

- **Le point 1 suffit à la sécurité.** Avec le retrait plafonné, le capital qui garantit les couvertures ne peut plus partir : un règlement perdant passe toujours. Les tests du filet (`contracts/pool/src/vault_net/`) le vérifient.
- **Le point 2 coûte aux LP junior qui sortent.** Le contrat calcule le prix de sortie d'une part junior à partir de la capacité. Retrancher la provision de la capacité baisse donc ce prix, même quand aucune couverture n'est ouverte. Un junior qui sort laisserait environ 1,48 % de la créance Safe aux juniors qui restent : par exemple 11 835 USDC pour 800 000 USDC de Safe.
- **Le point 2 ne sert à rien pendant le pilote.** Les dépôts Safe sont fermés au départ (décision T11 du 15/11). Sans part Safe, la provision vaut zéro.
- **Il casserait une vingtaine de tests existants**, sans gain de sécurité.

### Ce que ça implique

- **Un LP Safe peut être servi en retard.** Si les intérêts dépassent ce que le junior peut payer pendant que des couvertures sont ouvertes, sa valeur retirable cesse de monter au lieu de suivre les 4,5 %. Elle rattrape quand le junior se reconstitue (spreads encaissés, couvertures réglées). Rien n'est perdu tant qu'il ne retire pas.
- **Un LP Safe qui retire pendant ce retard abandonne sa part des intérêts non servis au junior.** Les parts Safe restantes gardent leur prix (la créance de chacune ne change pas) ; c'est la valeur junior (capital moins créance Safe) qui récupère la différence entre la créance du sortant et ce qu'il a touché. Ce n'est pas un risque de fonds, mais c'est à dire aux LP Safe. (Corrigé le 25/11 : la première version disait « aux LP Safe restants », ce que le code ne fait pas.)
- **La capacité peut tomber à zéro par le seul effet des intérêts** : plus aucune couverture ne s'ouvre et aucun junior ne peut sortir tant que des couvertures en cours n'ont pas été réglées. Le contrat ne l'affiche plus en négatif : elle reste à zéro.
- **Un dépôt Safe est refusé pendant un retard** (code 118), pour que le nouveau venu n'achète pas au prix plein une part qui vaut moins.
- **À dire à Jia** dans la note des changements, avec le reste de B5.

### Quand la revoir

À la réouverture des dépôts Safe, ou au branchement de la trésorerie placée (phase 4). Si les LP Safe deviennent nombreux, le retard de service sera plus fréquent, et la provision (ou des intérêts payés par la trésorerie plutôt que par le junior) redevient intéressante.

---

## D2. Pas de couverture nouvelle sur un pool réduit à moins de 1 USDC de parts

**Date :** 24/11/2026. **Décidé par :** Hugo. **Lot :** B6 (plancher de parts junior).

### Contexte

B6 interdit qu'un retrait laisse moins de 1 USDC de parts junior (le plancher) tant que du capital est réservé : sinon un attaquant gonfle le prix de la part et le déposant suivant y perd. Mais quand aucun capital n'est réservé, un LP peut légitimement descendre sous le plancher. Une couverture qui s'ouvrirait ensuite remettrait le pool dans la situation que B6 interdit.

### Décision

La capacité vaut zéro tant que le total des parts junior est sous le plancher. Aucune couverture ne s'ouvre sur un pool réduit à presque rien.

### Ce que ça implique

- Sur un pool vide ou presque, les ouvertures de couverture sont refusées (code 101, capacité insuffisante) jusqu'au prochain dépôt d'au moins 1 USDC.
- Aucun effet sur un pool normal.
- Le déposant suivant peut encore perdre l'arrondi d'une part, quelques centaines de stroops au plus (une fraction de centime) : c'est le coût résiduel accepté du plancher.
