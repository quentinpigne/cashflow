# ADR-0006 — Une seule connexion ; les membres du foyer sont des personnes, sans connexion

- **Statut** : Accepté
- **Date** : 2026-10-20 ; modèle des membres tranché le 2026-10-31 (spec 14)
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

**Membres du foyer (tranché le 2026-10-31, spec 14) :**
- Un **membre** (`Member`) est une **personne** du foyer, pas un utilisateur : il n'a pas de connexion.
  L'application garde **une seule connexion** (`app_user`, une ligne) ; tout ce qui est visible l'est pour
  elle. Il n'y a ni droits par membre, ni filtrage de sécurité, ni `AccountAccess`.
- Les membres servent à **affecter** : un **compte** (au plus un membre, son titulaire), une **ventilation**
  et un **emploi** ont au plus un membre (colonnes `member_id`) ; ce qui n'en a pas est au foyer. On choisit
  ensuite les membres à afficher dans l'accueil, les analyses et les projections. Les comptes joints à
  quotes-parts et les membres des biens, des prêts et des documents viendront plus tard (MBR-05).
- Catégories, règles, tiers, profils d'import, barèmes de taux, budgets, projets, scénarios de projection et
  récurrences restent **communs au foyer** (Q19).
- La rubrique **Foyer** s'active dans les réglages, désactivée par défaut : sans elle, l'application reste
  mono-personne, comme en V1.

Options écartées pour les membres :
- **Une connexion par membre, foyer transparent** — ❌ Des sessions et une gestion des mots de passe par
  membre, sans rien gagner en confidentialité puisque tout le monde voit tout.
- **Une connexion par membre, comptes privés** (`AccountAccess(account_id, user_id, role)`, modèle esquissé
  en V1) — ❌ Chaque requête devrait filtrer par membre (transactions, imports, documents, analyses,
  patrimoine) : chantier lourd et risque de fuite d'un écran à l'autre, pour un besoin qui n'existe pas.

## Conséquences

- La V1 reste simple : aucun écran ni aucune règle liés aux membres.
- Les membres n'ajoutent qu'une table et trois colonnes : aucune requête existante n'a à filtrer par membre
  pour des raisons de droits, et rien ne change quand la rubrique Foyer est désactivée.
- Le choix des membres affichés ne fait que restreindre les calculs existants (flux, soldes, projections) à
  des ventilations et des comptes ; toutes les cases cochées, les chiffres sont ceux du foyer (R-MBR-10).
- Le contexte d'appel (`RequestContext`) prévu en V1 n'a pas été construit : sans droits par membre, il
  n'est plus nécessaire.
