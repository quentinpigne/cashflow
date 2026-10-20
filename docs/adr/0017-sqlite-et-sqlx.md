# ADR-0017 — Base SQLite, accès par sqlx sans ORM

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : ADR-0009, ADR-0015, architecture, modèle de données

## Contexte

L'ADR-0009 prévoyait SQLite et sqlx. Deux alternatives ont été examinées avant de confirmer :
- une base serveur plus « solide » (MariaDB ou PostgreSQL) ;
- une véritable ORM (SeaORM 2) à la place de sqlx.

Le domaine se caractérise par :
- un seul utilisateur et un faible volume ;
- une persistance surtout faite d'**agrégats** (solde à date, consommation des budgets selon la ligne la plus
  spécifique, reports cumulés, rapports, recherche de correspondances) plutôt que de CRUD ;
- un cœur métier pur (ADR-0013) et un code produit majoritairement par IA.

Le propriétaire du projet tient à pouvoir **copier et déplacer** ses données facilement.

## Options envisagées

### Accès aux données
1. **SeaORM 2** — ✅ CRUD concis, entités et relations, migrations en Rust. ❌ Les requêtes d'agrégat
   finissent en SQL brut **non vérifié**. ❌ Risque que les entités ORM se diffusent dans le métier.
   ❌ Version majeure récente, mal connue des modèles d'IA. ❌ Une couche de plus.
2. **sqlx** — ✅ SQL **vérifié à la compilation** contre le schéma, y compris les agrégats complexes.
   ✅ Le module `db` construit directement les types du domaine. ✅ Migrations en SQL brut, avec
   accès complet aux fonctionnalités du moteur. ✅ Mature et bien connu. ❌ CRUD plus verbeux. ❌ Filtres
   optionnels moins confortables.

### Moteur
1. **MariaDB** — ❌ Modifications de schéma non transactionnelles ; aucun avantage décisif ici.
2. **PostgreSQL** — ✅ Typage riche, modifications de schéma complètes, invariant de ventilation
   vérifiable par un trigger différé, meilleure inférence sqlx. ❌ Conteneur, administration et montées
   de version majeure ; sauvegarde par dump ; tests d'intégration plus lents.
3. **SQLite** — ✅ **Un fichier** à copier, déplacer et sauvegarder à chaud. ✅ Rien à administrer.
   ✅ Tests en mémoire. ✅ Migrations transactionnelles. ❌ `ALTER TABLE` limité. ❌ Typage laxiste par
   défaut. ❌ Pas de contrainte inter-tables vérifiée en fin de transaction. ❌ Inférence sqlx de la
   nullabilité moins précise.

## Décision

**SQLite, accédé par sqlx (requêtes vérifiées à la compilation), sans ORM.**

Précautions obligatoires :
1. **Tables `STRICT`** partout, avec des contraintes `CHECK` qui remplacent les types absents (cf.
   modèle de données, § « Correspondance physique SQLite »).
2. **Connexion** : `journal_mode = WAL`, `foreign_keys = ON`, `synchronous = NORMAL`,
   `busy_timeout = 5000`. Une seule connexion en écriture. SQLite **embarqué** (`libsqlite3-sys` en
   version intégrée) pour garantir une version ≥ 3.37, requise pour `STRICT`.
3. **Invariant Σ ventilations** : garanti par le domaine, **et** contrôlé avant la validation de chaque
   transaction SQLite qui touche des transactions, en production comme en test. Une requête de contrôle
   limitée aux transactions modifiées annule la transaction SQLite en cas d'écart.
4. **Migrations qui recréent une table** (modification impossible avec `ALTER TABLE`) : on applique la
   procédure officielle de SQLite. Désactivation de `foreign_keys` **hors transaction**, puis
   création de la nouvelle table, copie, suppression de l'ancienne, renommage, `PRAGMA foreign_key_check`,
   validation, et réactivation. Avec sqlx, cela suppose une migration non transactionnelle qui gère sa
   propre transaction (directive `-- no-transaction`, dont le support est à vérifier dans la version de
   sqlx retenue). Chaque migration de ce type est testée sur une copie d'une base réaliste avant
   publication. La sauvegarde automatique avant migration (ADR-0015) sert de filet.
5. **Nullabilité** : quand sqlx infère mal la nullabilité d'une colonne calculée, on l'annote
   explicitement (`AS "col!"` ou `AS "col?"`), jamais par un `unwrap` côté Rust.

## Conséquences

- Déploiement inchangé (ADR-0015) : un conteneur applicatif, un volume `/data` qui contient
  `cashflow.db`. Sauvegarde = `VACUUM INTO`. Déplacer l'application = déplacer le volume.
- Pour inspecter la base : copier le fichier, ou utiliser `sqlite3` dans le conteneur.
- Passer plus tard à PostgreSQL demanderait de réécrire les requêtes (dialecte) ; les données
  migreraient par l'export JSON (ADR-0008). C'est un risque accepté.
