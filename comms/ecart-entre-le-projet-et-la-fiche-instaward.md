# Écart entre le projet et le dossier Instaward

Comparaison faite le 25/11 entre ce qu'on a promis dans le dossier Instaward (déposé le 13/11 auprès de Stellar Türkiye, 5 000 $) et ce qui est dans le projet : la version principale, plus la branche vault de Hugo. Rien n'a été modifié dans le projet.

Le mot « Instaward » n'apparaît nulle part dans le projet, ni dans la roadmap du 16/11. La roadmap suit le plan de 57 Blockchain, pas notre dossier Instaward. Or c'est sur le dossier qu'on sera jugés au bout des 30 jours.

## Ce qui suit bien le dossier

- **Corrections des tranches (Hugo, branche vault, pas encore dans la version principale)** : intérêts Safe qui faisaient passer le junior sous le capital réservé, contrôle manquant sur le retrait Safe, dernière part junior. C'est bien ce qu'on a promis.
- **Tests aléatoires du vault (Hugo)** : invariants et suites aléatoires. Ça couvre en partie les « property tests » promis, mais seulement pour le vault : rien encore pour le fixing.
- **Cours figé sans régler (Lancelot)** : le moyen de garder le cours d'échéance avant que l'oracle l'oublie est prêt. Il reste à l'exposer, c'est pour Clément.
- **Contrôle de fraîcheur du prix** ramené à 10 minutes à l'ouverture (Lancelot) : conforme.

## Ce qui a été ajouté et ne suit pas le dossier

1. **Le fixing.** Le dossier promet une **moyenne des cours sur environ 30 minutes, avec des bornes**, pour qu'on ne puisse pas manipuler le cours au moment de l'échéance. Le code fusionné le 24/11 prend **un seul cours** : celui de l'échéance, sinon le premier publié dans l'heure. Il n'y a ni moyenne ni bornes. C'est la décision du 16/11 (« cours unique à l'échéance »), prise sans tenir compte du dossier.
   - Rien n'est perdu : la fenêtre d'une heure, le cours figé et le constat d'absence se réutilisent. Il faudrait calculer la moyenne de quelques cours (par exemple 6 cours espacés de 5 minutes avant l'échéance) au lieu d'un seul, et refuser un cours trop loin de cette moyenne.
2. **La pause, le transfert d'admin et la mise à jour du code (Lancelot)** : c'est utile pour le mainnet et l'audit, que le dossier met **hors périmètre**. Ce n'est pas faux, mais ça a pris du temps sur ce qu'on doit livrer en 30 jours.
3. **La liste blanche des LP, active par défaut (Lancelot)** : ce n'est pas contraire au dossier, mais pour le pilote, chaque compte de démo devra être autorisé avant de déposer. Côté PME, il faudra que les 5 PME du pilote puissent ouvrir une couverture sans friction.

## Ce que le dossier promet et que rien ne prévoit

- **Livrable 2, régler et payer le fournisseur en une seule transaction** : swap USDC vers peso sur un échange Soroban (Soroswap ou Aquarius), montant maximal signé par la PME, pool de test USDC/peso créé par nous, bouton « Settle & pay supplier ». Ce n'est ni dans le code ni dans la roadmap, qui prévoit au contraire un solde à réclamer ensuite : c'est une autre direction.
- **Livrable 3, pilote avec au moins 5 PME réelles** : recrutement, sessions guidées, parcours d'accueil, confirmation téléchargeable, rapport public. Rien n'est planifié.
- **Redéploiement sur le vrai oracle Reflector du testnet** : le script d'installation déploie toujours notre faux oracle de démo.
- **Refus d'un dépôt LP juste avant un règlement gagnant** : promis, à faire par Lancelot une fois que Clément aura livré le solde réclamable.
- **Mode démo réservé à une version de test** : promis, à faire par Clément.
- **Règlement de secours sans cours** : promis, à faire par Clément.

## À trancher à trois, avant de coder la suite

1. **Fixing** : moyenne sur 30 minutes avec bornes comme promis, ou cours unique ? Si on garde le cours unique, il faut l'écrire à la responsable du chapitre (İrem Koçi).
2. **Paiement** : est-ce que régler et payer en une transaction remplace le solde à réclamer, ou s'y ajoute ? Les deux changent la façon dont le règlement verse l'argent.
3. **Calendrier** : on n'a pas encore la confirmation de l'Instaward. Est-ce qu'on refait la roadmap sur les 4 semaines du dossier (fixing, puis secours et redéploiement, puis paiement en une transaction et premières sessions, puis pilote, vidéos et rapport) ?
4. **Le pilote** : qui recrute les 5 PME, et quand ?

Clément ne commence pas le règlement de secours ni le robot tant que les points 1 et 2 ne sont pas tranchés.