# ADR-0013 — Cœur métier pur, isolé des entrées/sorties

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : architecture

## Contexte

La valeur de l'application est dans des calculs exigeants : soldes, intérêts par quinzaine, TRI,
dédoublonnage, correspondances, projections. Mêlés aux requêtes en base et au framework web, ils
deviennent difficiles à tester et à faire évoluer.

## Décision

Architecture **hexagonale** (ports et adaptateurs) :
- **Cœur métier** : types (`Money`, `Rate`, identifiants), entités, invariants et **moteurs** sous forme
  de fonctions pures. Aucune dépendance vers la base, le système de fichiers, le réseau ou l'horloge.
- **Cas d'usage (application)** : orchestrent le cœur, ouvrent et ferment les transactions de base de
  données, appellent les ports.
- **Adaptateurs** : persistance, lecteurs de fichiers, OCR, interface (UI/API), horloge.

## Conséquences

- La majorité des tests porte sur le cœur, sans base ni mocks : rapides et déterministes.
- Les specs se traduisent presque littéralement en tests (une règle `R-*` → un ou plusieurs tests).
- Changer d'interface ou de base n'affecte pas le cœur.
- Un peu plus de code d'assemblage (mappings entre entités et persistance) : c'est un coût accepté.
