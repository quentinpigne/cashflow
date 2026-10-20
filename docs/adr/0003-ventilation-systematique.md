# ADR-0003 — Ventilation systématique des transactions

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : spec 02, spec 05, spec 08 ; modèle de données

## Contexte

Une transaction doit pouvoir être répartie sur plusieurs postes (catégories, voire budgets). La
plupart des transactions n'ont pourtant qu'un seul poste.

## Options envisagées

1. **Catégorie sur la transaction + table de ventilation optionnelle** — ✅ le cas simple reste simple
   en base. ❌ Deux chemins de lecture partout (transaction ventilée ou non) : chaque agrégat, chaque
   rapport, chaque règle doit gérer les deux cas, et les bugs naissent dans cette dualité.
2. **Ventilation systématique : toute transaction a ≥ 1 `Split`, la catégorie n'existe que sur le `Split`**
   — ✅ un seul chemin : tous les agrégats se font sur les ventilations. ❌ Une ligne de plus par
   transaction (négligeable).

## Décision

Option 2. La catégorie (et le futur rattachement budgétaire) vit **uniquement** sur `Split`.
L'invariant `Σ split.amount = transaction.amount` est garanti par le cœur métier à chaque écriture,
vérifié par des tests de propriété et, si la base le permet simplement, par une contrainte différée
ou un contrôle en fin de transaction.

## Conséquences

- L'interface masque la ventilation quand il n'y a qu'un seul `Split`.
- Toute écriture de transaction est une écriture « transaction + ventilations », dans une même
  transaction de base de données.
- Les agrégats (dépenses par catégorie, budgets) ne lisent que la table des ventilations, jointe aux
  transactions pour la date et le compte.
