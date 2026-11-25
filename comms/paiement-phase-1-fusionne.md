# Paiement, phase 1 : fusionné dans la version principale

Fusionné le 25/11 après la relecture de Hugo. Tous les tests passent : 226 sur la version normale du contrat, 230 sur la version démo.

## Ce qui a été fait, et ce que ça change

- **Le règlement ne dépend plus du compte de la PME.** Le règlement crédite un solde à son nom au lieu de lui envoyer l'argent ; elle le réclame, ou le robot le lui envoie. Une PME qui supprime sa ligne USDC ne peut plus bloquer le règlement pour s'en sortir sans perte (faille relevée par Hugo).
- **Le cours d'échéance peut être figé** sans régler : le robot le fait avant chaque règlement, car Reflector oublie ses cours au bout de 24 h.
- **Une clôture de secours, réservée à l'admin**, ferme sans gain ni perte une couverture dont le cours est définitivement perdu. Réservée à l'admin après la relecture de Hugo : ouverte à tous, ce serait une option gratuite pour le perdant.
- **Taux limite à l'ouverture** : la PME signe le taux du devis ; si le cours a bougé, la couverture est refusée et rien n'est prélevé. L'ouverture prend donc un argument de plus : le front et les scripts devront le passer au redéploiement.
- **Liste blanche des PME du pilote**, active par défaut, et limites par couverture : montant, durée, nombre de couvertures ouvertes. Les scripts autorisent les PME de démo.
- **Paiement du fournisseur en une transaction** (dossier Instaward, livrable 2) : la PME signe une seule fois ; le contrat règle, échange ses USDC contre exactement le montant de la facture en pesos sur Soroswap sans dépasser son maximum ni sa date limite, et paie le fournisseur. Tout ou rien.
- **Le mode démo** (échéances accélérées) n'existe plus que dans une version compilée exprès : les couvertures de production ont toujours leur vraie échéance.
- **Tests aléatoires du règlement** (dossier Instaward, livrable 1) : des scénarios rejouables vérifient après chaque étape que l'argent est conservé et que le pool et les PME jouent à somme nulle. Vérifiés en y introduisant un bug exprès, qu'ils ont détecté.
- Les calculs démesurés renvoient une erreur numérotée au lieu de planter. Codes d'erreur 207 à 220, à traduire dans le front.

## Pour les autres

- **Hugo** : le vault peut être fusionné. Côté front : les nouveaux codes d'erreur, l'argument de taux limite à l'ouverture, le bouton de réclamation du solde, le bouton « régler et payer le fournisseur ».
- **Lancelot** : la fonction côté vault pour la part du protocole ; le point de Hugo sur les cours entre 24 et 25 h après l'échéance ; autoriser le LP de démo dans le script d'installation.

## Ce qui reste

- **Part du protocole** : côté paiement après la fonction de Lancelot. Proposition : un pourcentage du spread payé à l'ouverture, taux à 0 tant qu'il n'est pas décidé.
- **Au redéploiement** : créer et financer la paire USDC/peso sur Soroswap, poser la route de paiement, essayer le paiement en une transaction sur le vrai Soroswap (testé jusqu'ici avec un routeur de test à l'interface identique), et une couverture d'un jour en version normale réglée par le robot.
- **Limite connue** : comme tout échange sur Soroban, si les réserves de la paire bougent entre la préparation et l'exécution, la transaction échoue sans rien faire et il faut la relancer.