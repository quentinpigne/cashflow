# ADR-0018 — Environnements, livraison et intégration continue

- **Statut** : Proposé
- **Date** : 2026-10-20
- **Concerne** : ADR-0008, ADR-0015, ADR-0017, architecture.md, roadmap.md

## Contexte

L'ADR-0015 décrit l'empaquetage (une image Docker, un binaire qui sert l'API et l'interface), le HTTPS
par Caddy, l'authentification et les sauvegardes. Il ne dit pas combien d'environnements existent, comment
une version arrive sur le serveur, ni ce que vérifie l'intégration continue.

Contraintes :
- Le dépôt est hébergé sur **GitHub**. Le serveur personnel n'est joignable que depuis le **réseau local**.
- Le code est écrit **en totalité par une IA** (Claude Code) sur le Mac. Le propriétaire du projet cadre,
  relit et teste ; il ne code pas.
- Le propriétaire ne veut maintenir **qu'un seul environnement** : la production. Il y teste les
  nouvelles versions, sur ses vraies données.

## Options envisagées

1. **Production + recette sur le serveur.** ✅ Les essais ne touchent pas les vraies données.
   ❌ Un second environnement à maintenir et à alimenter, que le propriétaire ne veut pas.
2. **Production seule, livrée sans garde-fou.** ✅ Simple. ❌ Une régression ou une migration ratée
   atteint directement les vraies données.
3. **Production seule, protégée en amont et en aval.** Avant la livraison, l'IA vérifie tout sur le Mac
   et la CI bloque ce qui ne passe pas. Après la livraison, une sauvegarde est faite avant chaque
   déploiement et un retour arrière est toujours possible. ✅ Un seul environnement, risque contenu.
   ❌ Discipline à tenir sur les vérifications locales.

## Décision

Option 3.

### Environnements
- **Un seul environnement : la production**, sur le serveur, selon l'ADR-0015.
- Le **poste de développement** (le Mac) n'est pas un environnement : la toolchain Rust et Node y est
  installée en natif, **sans Docker**, et l'application s'y lance sur une **base jetable** remplie de
  **données fictives** (jeu de démonstration versionné, jamais de donnée réelle).

### Vérifications avant livraison (par l'IA, sur le Mac)
Avant de présenter une modification comme terminée, l'IA :
1. fait passer le lint et tous les tests ;
2. lance l'application sur la base de démonstration et vérifie le parcours modifié dans un navigateur
   (tests de bout en bout Playwright quand ils existent) ;
3. pour une migration, l'applique sur une copie de la base de démonstration et vérifie le retour à un
   état cohérent (ADR-0017).

### Intégration continue (GitHub Actions)
- À chaque push et à chaque pull request : lint, tests, vérification qu'`openapi.json`, le client
  généré et `.sqlx/` sont à jour, construction de l'image Docker. Tous les contrôles sont **bloquants**.
- La CI **ne déploie pas** : le serveur n'est pas joignable depuis GitHub, et ne doit pas l'être.

### Livraison
- Le déploiement se fait **sur le serveur**, par une seule commande : récupération de la version
  (`git pull` d'un commit dont la CI est verte), sauvegarde de la base, construction de l'image,
  redémarrage.
- Chaque image est **étiquetée par son commit**. L'image précédente est conservée.
- **Retour arrière** : relancer l'image précédente et, si une migration a eu lieu, restaurer la
  sauvegarde prise juste avant le déploiement.

## Conséquences

- Aucun environnement de recette à maintenir ; le propriétaire teste en production.
- La qualité des vérifications locales de l'IA et de la CI devient la principale protection des
  données : les tests de propriété, les golden files et les tests de bout en bout ne sont pas optionnels.
- Un **jeu de données de démonstration** fictif est à créer dès la phase 1 et à faire évoluer avec le
  modèle ; il sert aussi aux tests de bout en bout.
- Une sauvegarde avant chaque déploiement s'ajoute à la sauvegarde quotidienne et à celle d'avant
  migration (ADR-0015).
- Le Mac doit disposer de la toolchain Rust et de Node ; la liste des outils et des commandes
  (`justfile` ou équivalent) reste à fixer dans `architecture.md`.
- Le serveur doit disposer de Docker et de Git.
