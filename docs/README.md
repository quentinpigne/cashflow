# Cashflow — Dossier de conception

Application de gestion de finances personnelles : comptes, transactions ventilées,
budgets, import et rapprochement de relevés bancaires, épargne à taux fixe et à
taux variable avec projections.

Ce dossier contient **toute la conception de l'application avant implémentation**.
Il est autonome : il peut être extrait tel quel pour initialiser un nouveau dépôt.

> **Statut : conception terminée (phase 0).** Tous les ADRs sont acceptés et toutes les specs validées.
> Aucune ligne de code n'est écrite ; prochaine étape : phase 1 de la [roadmap](roadmap.md).
> Questions encore ouvertes, non bloquantes : [questions-ouvertes.md](questions-ouvertes.md).

## Par où commencer

| Je veux… | Lire |
|---|---|
| Comprendre le produit et ses limites | [vision.md](vision.md) |
| Connaître le vocabulaire métier | [glossaire.md](glossaire.md) |
| Voir le modèle de données complet | [modele-de-donnees.md](modele-de-donnees.md) |
| Comprendre l'architecture cible | [architecture.md](architecture.md) |
| Voir les écrans et la navigation | [ecrans.md](ecrans.md) |
| Connaître le format d'export | [export.md](export.md) |
| Connaître l'ordre de réalisation | [roadmap.md](roadmap.md) |
| Voir ce qui reste à trancher | [questions-ouvertes.md](questions-ouvertes.md) |
| Savoir quoi récupérer de l'ancien projet | [heritage.md](heritage.md) |
| Lire les règles et features d'un domaine | [specs/](specs/) |
| Comprendre pourquoi un choix a été fait | [adr/](adr/) |

## Structure

```
docs/
├── README.md          ← ce fichier
├── vision.md
├── glossaire.md
├── architecture.md
├── ecrans.md
├── export.md
├── modele-de-donnees.md
├── roadmap.md
├── questions-ouvertes.md
├── heritage.md
├── specs/             ← une spec par domaine : concepts, règles, features
│   ├── 01-comptes.md
│   ├── 02-transactions.md
│   ├── 03-import.md
│   ├── 04-rapprochement.md
│   ├── 05-budgets.md
│   ├── 06-epargne.md
│   ├── 07-projections.md
│   └── 08-tableau-de-bord.md
└── adr/               ← Architecture Decision Records
    ├── README.md      ← index + processus
    ├── template.md
    └── NNNN-*.md
```

Les consignes pour Claude Code sont dans `CLAUDE.md`, à la racine du dépôt.

## Conventions de la documentation

- **Langue** : documentation en français ; identifiants (entités, champs, enums) en anglais.
  La correspondance est dans le [glossaire](glossaire.md).
- **Identifiants de traçabilité** :
  - Règles métier : `R-<DOMAINE>-<n>` (ex. `R-TRX-3`). Chaque règle doit avoir au moins un test.
  - Features : `<DOMAINE>-<nn>` (ex. `IMP-04`), avec une priorité `MVP`, `V1` ou `V2`.
  - Codes domaine : `CPT` comptes, `TRX` transactions, `IMP` import, `RAP` rapprochement,
    `BUD` budgets, `EPA` épargne, `PRJ` projections, `TDB` tableau de bord, `BIE` biens, `PRE` prêts.
- **Statut des documents** : chaque spec indique en tête `Brouillon`, `Revue` ou `Validée`.
- **Décisions** : toute décision structurante passe par un ADR (voir [adr/README.md](adr/README.md)).
