# Point d'étape Lancelot, phase 1 : ce qui change pour vous

Tout ce qui suit est sur ma branche `lancelot/phase-1`, testé (198 tests verts, le contrat compile), mais **pas encore poussé** : j'attends ton accord, Hugo.

## En bref

- Fini côté pricing et admin : B7, B8, B1, B10, et B9 pour la moitié vault.
- Une relecture complète a été faite, et ses corrections sont intégrées.
- Aucune signature publique existante n'a changé. Il y a des fonctions nouvelles, et deux fonctions internes ont changé de forme.
- **Clément**, plusieurs de tes lots s'appuient sur ce travail : la section qui te concerne est juste en dessous.

## Pour Clément

1. **Deux fonctions internes du pricing prennent un argument de plus.** La conversion en USDC et le calcul de la marge prennent maintenant l'environnement en premier argument : elles renvoient une erreur numérotée (313) au lieu de planter sur un montant démesuré. J'ai adapté la seule ligne concernée dans ton ouverture de couverture. Sur ta branche, tes appels ne compileront plus au merge tant qu'ils n'ajoutent pas cet argument.

2. **Le règlement a changé (B1).**
   - Il prend le cours en vigueur à l'échéance s'il a moins de 15 minutes. Sinon, il prend le premier cours publié dans l'heure qui suit.
   - Tant qu'aucun cours n'est disponible, il échoue avec l'erreur 312 et la couverture reste ouverte. Le cas « cours trop ancien » ne renvoie plus 302.
   - Un cours publié juste après l'échéance n'est utilisable qu'environ 5 minutes plus tard : ton robot ne doit pas conclure à un échec s'il passe trop tôt.
   - J'ai changé une ligne dans ton règlement, et adapté deux de tes tests de règlement dont le comportement changeait. Aucune assertion retirée.

3. **À toi d'exposer deux points d'entrée, avant la mise en production.**
   - **Figer le cours sans régler.** La fonction du pricing est prête : elle fige le cours d'échéance d'une couverture et le renvoie. On peut l'appeler plusieurs fois sans risque, et elle répond 312 s'il n'y a pas encore de cours. Il reste à l'exposer dans l'interface, sans signature, et à la faire appeler par ton robot **dans les 24 heures qui suivent chaque échéance** : Reflector oublie ses cours au bout de 24 h.
   - **Régler sans cours, en secours.** Si personne n'a figé le cours ni réglé à temps, la couverture ne peut plus être réglée normalement. La condition à tester pour ce secours est déjà écrite côté pricing : elle dit si l'absence de cours est constatée.

4. **Ta moitié de B9 (liste blanche des PME).** J'ai créé un rôle partagé « LP ou PME » et une lecture des limites du pilote. La partie PME renvoie l'erreur 405 (« pas encore implémenté ») en attendant que tu la branches, à trois endroits de l'interface. Tu y ajouteras tes propres champs (plafonds par couverture, durée, nombre de couvertures ouvertes). Dis-moi si la forme te va avant de coder.

5. **Pause (B10).** Ouvrir une couverture est refusé quand le contrat est en pause (erreur 400). Régler et clore une couverture restent toujours possibles.

## Pour Hugo

1. **Nouvelles fonctions dans l'interface**, utiles au front :
   - administration : proposition et acceptation d'un nouvel admin, annulation, clé de garde, pause et reprise, mise à jour du code ;
   - lectures : l'admin actuel, l'admin proposé, la garde, l'état de pause (pour ton bandeau), les limites du pilote, le notionnel total, et si une adresse est autorisée ;
   - réglages du pilote : autoriser une adresse, activer la liste blanche, poser un plafond de notionnel.

2. **Nouveaux codes d'erreur à traduire dans le front :**

   | Code | Sens |
   |---|---|
   | 114 | Ce compte n'est pas autorisé à déposer pendant le pilote |
   | 115 | Le plafond de montant couvert du pilote est atteint |
   | 313 | Montant ou cours hors de toute plage réaliste |
   | 400 | Le contrat est en pause |
   | 401 | Aucun nouvel admin en attente |
   | 402 | Seule la clé de garde ou l'admin peut mettre en pause |
   | 403 | L'oracle ne publie pas ses cours avec 14 décimales |
   | 404 | La garde et l'admin doivent être deux comptes différents |
   | 405 | Fonction pas encore disponible |

   Le cours est maintenant jugé trop ancien au-delà de **10 minutes** au lieu de 15 (erreur 302 au devis et à l'ouverture).

3. **Ton fichier vault, à relire** comme prévu par la roadmap. J'y ai écrit B9 : la liste blanche des LP, active par défaut, et le compteur du notionnel. Dans tes tests existants, je n'ai fait qu'autoriser les LP avant leurs dépôts, et remplacer le faux oracle par le mock (il doit maintenant répondre « 14 décimales »). Aucune assertion retirée. Le code 113 reste libre pour B3, et 116 pour toi.

4. **Pause :** dépôts et ouvertures refusés. Retraits, règlements et lectures toujours possibles : on ne bloque jamais l'argent des gens.

## Pour le redéploiement (semaine 49)

- Autoriser les LP de démo avant leurs dépôts, sinon ils échouent avec le code 114.
- Poser la clé de garde après le déploiement : le constructeur ne la pose pas.
- Vérifier une vraie mise à jour du code sur testnet : elle n'est pas testée en local.
- L'oracle doit avoir 14 décimales. C'est le cas du mock et de Reflector.

## Ce que j'attends de vous

- **Hugo** : ton accord pour pousser ma branche, et ta relecture du vault.
- **Clément** : ton avis sur la forme du rôle partagé et des limites du pilote, avant ta moitié de B9 ; puis les deux points d'entrée (figer le cours, régler en secours).
- **À trancher à trois** : la fenêtre de 24 h pour figer le cours, à écrire dans la spec ; les échéances le week-end, que Reflector permet ; qui s'occupe du mock-oracle et de ses scripts.
