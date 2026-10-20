# ADR-0001 — Montants en centimes entiers, taux en millionièmes

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : modèle de données, toutes les specs

## Contexte

Un nombre à virgule flottante binaire ne représente pas exactement la plupart des montants décimaux
(0,1 + 0,2 ≠ 0,3) : les sommes dérivent de quelques fractions de centime, ce qui est incompatible avec
un rapprochement exact au centime (spec 04). Il faut aussi stocker des taux d'intérêt (1,7 %, 0,85 %)
et faire des calculs d'intérêts intermédiaires plus précis que le centime.

La base est SQLite (ADR-0017), qui n'a **pas** de vrai type décimal : `NUMERIC` peut y être stocké
en flottant.

## Options envisagées

1. **Décimal (`DECIMAL(12,2)` en base, type décimal dans le langage)** — ✅ lisible. ❌ Pas de support
   natif dans SQLite. ❌ La précision dépend de la configuration de chaque couche.
2. **Entier en unités mineures (centimes)** — ✅ exact, portable dans toutes les bases et tous les
   langages, sommes triviales. ❌ Conversion à l'affichage et à la saisie, attention aux devises
   sans centimes (hors périmètre, cf. vision).
3. **Chaîne de caractères décimale** — ❌ Pas d'agrégation en SQL.

## Décision

- Les **montants** sont stockés et manipulés comme des **entiers signés 64 bits en unités mineures**
  de leur devise, enveloppés dans un type `Money` du cœur métier (le montant et sa devise). Le nombre
  de décimales d'une devise est celui de la norme ISO 4217 : 2 pour l'euro (centimes), 0 pour le yen,
  3 pour le dinar tunisien. Seul l'euro est utilisé tant que le multi-devise est hors périmètre
  (vision), mais aucun code ne doit supposer « 2 décimales » en dur : il interroge la devise. Aucune
  opération sur `Money` ne passe par un flottant, y compris la lecture des fichiers d'import (R-IMP-2).
- Les **quantités** et **cours unitaires** (nombre de parts, valeur liquidative, cours d'un titre),
  nécessaires à un futur suivi ligne à ligne des portefeuilles, ne sont **jamais** des `Money`. Ce sont
  des types `Quantity` et `UnitPrice` en décimal exact, à précision libre, stockés en texte décimal.
  Seul le résultat d'une valorisation (quantité × cours) devient un `Money`, arrondi à l'unité mineure
  de la devise. Les soldes et valorisations publiés par les établissements, eux, sont toujours à l'unité
  mineure.
- Les **taux** sont stockés comme des **entiers en millionièmes** (1,7 % = 17 000).
- Les **calculs intermédiaires** (intérêts par quinzaine, projections, ventilation en pourcentages)
  utilisent une arithmétique **décimale exacte** (ou rationnelle) du langage choisi, et ne sont
  arrondis au centime qu'aux points définis par les specs. Le mode d'arrondi par défaut est « au plus
  proche, demi vers le haut » (`HALF_UP`), sauf mention contraire.

## Conséquences

- Les tests de propriété peuvent vérifier les invariants de somme par égalité stricte.
- Tout calcul qui produit des centimes fractionnaires doit dire où et comment il arrondit ; les specs
  06 et 07 le précisent.
- Critère de choix de la stack (ADR-0009) : disposer d'une bibliothèque décimale exacte fiable.
