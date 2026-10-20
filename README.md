# Cashflow

Application personnelle de gestion de finances : comptes, transactions ventilées,
budgets, import et rapprochement de relevés bancaires, épargne à taux fixe et à
taux variable avec projections.

Mono-utilisateur, auto-hébergée sur le réseau local : les données ne quittent pas
la machine sans action volontaire.

## Statut

La conception est terminée (phase 0) et aucune ligne de code n'est encore écrite.
La prochaine étape est la phase 1 de la [roadmap](docs/roadmap.md) : un registre exact
des comptes.

## Stack

- **Backend** : Rust (axum, sqlx + SQLite), organisé en deux crates : `domain` (calculs purs) et `server`.
- **Frontend** : Angular et Angular Material, dans `web/`.
- **Déploiement** : un seul binaire qui sert l'API et la SPA, une image Docker sur un serveur personnel,
  en HTTP sur le réseau local pour la V1.

Le détail est dans [docs/architecture.md](docs/architecture.md) et dans les ADR 0009, 0013 et 0015 à 0018.

## Documentation

Toute la conception se trouve dans [`docs/`](docs/README.md) :

- [vision](docs/vision.md) : objectifs, non-objectifs et principes produit ;
- [glossaire](docs/glossaire.md) et [modèle de données](docs/modele-de-donnees.md) ;
- [specs](docs/specs/) : règles et features par domaine ;
- [ADR](docs/adr/README.md) : les décisions d'architecture.

Les consignes pour Claude Code sont dans [CLAUDE.md](CLAUDE.md).

## Commandes

Le projet n'utilise pas d'outil de commandes : les commandes `cargo` et `npm` (tests, lint,
génération du client d'API, déploiement) sont listées dans
[docs/architecture.md](docs/architecture.md), § « Commandes ».
