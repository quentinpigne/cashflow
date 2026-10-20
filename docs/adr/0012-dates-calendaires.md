# ADR-0012 — Dates calendaires sans fuseau

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : modèle de données, specs 02 à 07

## Contexte

Une opération bancaire a lieu **un jour donné**, pas à un instant : une conversion de fuseau peut la
faire glisser d'un jour, et donc d'une quinzaine, d'un mois, voire d'un relevé.

## Décision

- Les dates métier (opération, valeur, valorisation, point de contrôle, taux, récurrences) sont des
  **dates calendaires** (`YYYY-MM-DD`), sans heure ni fuseau.
- Les **instants** (`Timestamp`, UTC) sont réservés à l'audit : création, modification, validation d'import.
- « Aujourd'hui » est fourni au cœur métier par une **horloge injectée**, jamais lue directement, afin
  que les calculs dépendants de la date soient testables.

## Conséquences

- Pas d'ambiguïté de fuseau dans les calculs d'intérêts ou de période.
- L'ordre de deux transactions du même jour se fait par ordre de création (UUIDv7, ADR-0011).
