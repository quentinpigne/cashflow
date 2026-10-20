# Architecture Decision Records

Un ADR documente **une** décision structurante : son contexte, les options envisagées, le choix
retenu et ses conséquences. Il sert à comprendre plus tard *pourquoi* le projet est comme il est.

## Processus

1. Copier [template.md](template.md) en `NNNN-titre-en-kebab-case.md` (numéro suivant).
2. Statut initial : `Proposé` (une option recommandée) ou `À décider` (plusieurs options, pas de choix).
3. Après revue par le propriétaire du projet : `Accepté`.
4. Un ADR `Accepté` n'est plus modifié sur le fond. Pour changer d'avis, on crée un nouvel ADR, et
   l'ancien passe à `Remplacé par ADR-XXXX`. Seules les corrections de forme sont permises.
5. Mettre à jour l'index ci-dessous.

## Index

| N° | Titre | Statut |
|---|---|---|
| [0001](0001-montants-en-entiers.md) | Montants en centimes entiers, taux en millionièmes | Accepté |
| [0002](0002-solde-calcule-et-points-de-controle.md) | Solde calculé et points de contrôle | Accepté |
| [0003](0003-ventilation-systematique.md) | Ventilation systématique des transactions | Accepté |
| [0004](0004-virements-internes.md) | Virements internes comme lien entre deux transactions | Accepté |
| [0005](0005-import-en-deux-temps.md) | Import en deux temps et dédoublonnage par empreinte | Accepté |
| [0006](0006-mono-utilisateur.md) | Mono-utilisateur en V1, multi-membres du foyer à terme | Accepté |
| [0007](0007-services-tiers-optionnels.md) | Services tiers (OCR) explicites et optionnels | Accepté |
| [0008](0008-export-ouvert.md) | Export complet dans un format ouvert | Accepté |
| [0009](0009-choix-de-la-stack.md) | Choix de la stack technique : Rust (axum, sqlx/SQLite) + React/TypeScript | Accepté, front remplacé par 0019 |
| [0010](0010-epargne-variable-par-valorisations.md) | Épargne variable suivie par valorisations | Accepté |
| [0011](0011-identifiants-uuidv7.md) | Identifiants UUIDv7 | Accepté |
| [0012](0012-dates-calendaires.md) | Dates calendaires sans fuseau | Accepté |
| [0013](0013-coeur-metier-pur.md) | Cœur métier pur, isolé des entrées/sorties | Accepté |
| [0014](0014-modele-de-budget.md) | Modèle de budget : lignes par catégorie, projets et objectifs d'épargne | Accepté |
| [0015](0015-deploiement-auto-heberge.md) | Déploiement auto-hébergé sur le réseau local | Accepté, HTTPS remplacé par 0020 |
| [0016](0016-contrat-api-openapi.md) | Une API REST unique, décrite en OpenAPI, avec un client généré | Accepté |
| [0017](0017-sqlite-et-sqlx.md) | Base SQLite, accès par sqlx sans ORM | Accepté |
| [0018](0018-environnements-livraison-et-ci.md) | Environnements, livraison et intégration continue | Proposé |
| [0019](0019-front-angular.md) | Interface en Angular avec Angular Material | Accepté |
| [0020](0020-http-en-v1.md) | HTTP simple en V1, HTTPS par axum ensuite | Accepté |
