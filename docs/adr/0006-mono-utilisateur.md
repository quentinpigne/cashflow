# ADR-0006 — Mono-utilisateur en V1, multi-membres du foyer à terme

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : architecture, modèle de données, ADR-0015

## Contexte

En V1, l'application sert une seule personne. À terme, **chaque membre de la famille** devra pouvoir
gérer ses comptes et son patrimoine, ce qui implique des comptes partagés (comptes joints) et des
droits par membre.

Construire le multi-membres dès la V1 multiplierait le travail (droits, partage, vues par membre et
par foyer) avant même que le cœur du produit existe. À l'inverse, l'ignorer complètement rendrait la
transition plus coûteuse qu'elle ne devrait l'être.

## Options envisagées

1. **Mono-utilisateur strict, sans aucune préparation** — ❌ La transition toucherait l'authentification,
   les sessions et toutes les requêtes dispersées dans le code.
2. **Multi-membres dès la V1** — ❌ Coût élevé, et des questions produit pas encore mûres (budgets
   personnels ou familiaux, visibilité des comptes).
3. **Mono-utilisateur en V1, avec une préparation peu coûteuse** — ✅ Pas de complexité fonctionnelle
   en V1, et une transition qui reste une évolution, pas une réécriture.

## Décision

Option 3.

**En V1 :**
- Une table `app_user` existe **dès la V1**, avec une seule ligne créée par `cashflow set-password`.
  Les sessions référencent un `user_id`. Les modalités d'authentification sont dans ADR-0015.
- Les données métier n'ont **pas** de propriétaire en V1 : pas de colonne `owner_id`, aucun filtre par
  utilisateur.
- **Tout accès aux données passe par le module `db`** (ADR-0013). Aucun service ni aucune route
  n'exécute de requête en dehors de lui, ce qui permettra d'ajouter le filtrage par membre à un seul
  endroit.
- Les services reçoivent un **contexte d'appel** (`RequestContext`, qui contient l'utilisateur
  courant), même s'ils ne s'en servent pas encore.

**Modèle cible esquissé (non implémenté, à confirmer avant les travaux) :**
- `AccountAccess(account_id, user_id, role)` avec un rôle `OWNER` ou `VIEWER` : un compte joint est
  accessible à plusieurs membres. Un membre ne voit que les comptes auxquels il a accès, et toutes les
  données qui en découlent (transactions, ventilations, imports, points de contrôle, valorisations).
- Catégories, règles de catégorisation, profils d'import, barèmes de taux : **partagés par le foyer**.
- Budgets, projets, scénarios de projection : rattachés à un membre **ou** au foyer. À trancher le
  moment venu (questions ouvertes).

## Conséquences

- La V1 reste simple : aucun écran ni aucune règle liés aux membres.
- La migration vers le multi-membres consistera à créer `AccountAccess` (en attribuant tous les comptes
  existants au premier utilisateur), à ajouter un propriétaire aux entités rattachées à un membre, et à
  appliquer le filtrage dans les repositories.
- Le patrimoine, le taux d'épargne et les budgets devront alors exister en deux vues : par membre et
  pour le foyer. Les définitions de la spec 08 seront à étendre.
