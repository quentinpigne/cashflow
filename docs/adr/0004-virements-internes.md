# ADR-0004 — Virements internes comme lien entre deux transactions

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : spec 02, spec 03, spec 07, spec 08

## Contexte

Un virement du compte courant vers le Livret A apparaît sur les deux relevés. Si on le traite comme
deux transactions indépendantes, il compte à la fois comme une dépense et comme un revenu : le taux
d'épargne et les budgets sont faussés. Les deux jambes arrivent souvent par deux imports distincts,
à des dates qui peuvent différer d'un ou deux jours.

## Options envisagées

1. **Catégorie spéciale « Virement »** — ✅ simple. ❌ Aucune garantie que les deux jambes existent et
   se compensent. ❌ Ne distingue pas un virement vers un compte suivi d'un versement à un tiers.
2. **Transaction unique à deux comptes (from/to)** — ✅ cohérence garantie. ❌ Casse le modèle « une
   transaction = un compte », complique l'import (chaque relevé n'en voit qu'une moitié), le
   rapprochement et le solde calculé.
3. **Deux transactions ordinaires reliées par une entité `Transfer`** — ✅ chaque compte garde ses
   transactions (import, rapprochement et solde inchangés). ✅ On peut lier après coup deux
   transactions importées séparément. ❌ Il faut maintenir l'invariant de lien.

## Décision

Option 3. `Transfer(outflow_transaction_id, inflow_transaction_id)`, avec montants opposés, comptes
distincts, même devise. Les jambes ont une ventilation unique sans catégorie et sont exclues des
revenus, des dépenses et des budgets. Un flux vers un compte **non suivi** n'est pas un virement
interne : c'est une transaction catégorisée.

Quand on marque une transaction comme virement vers ou depuis un compte suivi et qu'aucune jambe
opposée candidate n'existe, l'application **crée automatiquement la jambe opposée** sur ce compte
(R-TRX-36). C'est indispensable pour les comptes dont aucun relevé n'est importé, comme une assurance
vie alimentée depuis le compte courant.

## Conséquences

- L'import propose des correspondances de virement entre comptes (R-TRX-35, R-IMP-14).
- Le patrimoine est invariant par virement interne, ce qui sert de test.
- Les virements vers un compte d'épargne alimentent les objectifs d'épargne du budget (ADR-0014, R-BUD-31),
  sans jamais devenir des dépenses.
