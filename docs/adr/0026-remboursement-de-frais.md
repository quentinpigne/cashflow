# ADR-0026 — Remboursement de frais porté par la ventilation de l'entrée d'argent

- **Statut** : Accepté
- **Date** : 2026-10-29
- **Concerne** : spec 02, spec 05, spec 08, modèle de données, écrans E1 et E7

## Contexte

Une entrée d'argent rembourse souvent tout ou partie d'une dépense : l'assurance maladie rembourse 17,50 € d'une
consultation de 25 €, puis la mutuelle une partie du reste ; chaque mois, les notes de frais remboursent une part
du forfait téléphone et une part de l'essence. L'argent est déjà bien compté : une ventilation positive sur une
catégorie de dépense vient en déduction de cette catégorie (R-TRX-13). Ce qui manque est le **lien** entre
l'entrée et la ou les dépenses précises qu'elle rembourse, et le suivi de ce qu'on attend encore.

Contraintes :
- une entrée peut rembourser plusieurs dépenses, et une dépense être remboursée par plusieurs entrées, en tout
  ou partie ;
- les ventilations d'une transaction sont remplacées, avec de nouveaux identifiants, à chaque modification de
  la transaction (`replace_splits`) ;
- un mois clos ne doit pas changer quand un remboursement arrive (budgets, reports, analyses).

## Options envisagées

1. **Table de liens entrée ↔ dépense avec un montant** — ✅ symétrique. ❌ Le montant remboursé double celui des
   ventilations de l'entrée : deux vérités à garder égales. ❌ La catégorie qui reçoit la déduction reste à
   déterminer à part.
2. **Lien porté par la ventilation de l'entrée, vers une ventilation de la dépense** — ✅ un remboursement est
   une ventilation : montant, catégorie et lien au même endroit. ❌ Les ventilations de la dépense changent
   d'identifiant à chaque modification de la dépense : le lien se perdrait.
3. **Lien porté par la ventilation de l'entrée, vers la transaction de la dépense** — ✅ comme l'option 2, et
   l'identifiant d'une transaction est stable. ❌ Une dépense ventilée sur plusieurs catégories ne dit pas
   laquelle est remboursée : la ventilation de l'entrée le dit par sa propre catégorie.

## Décision

Option 3 :
- `Split.refunds_transaction_id` (optionnel) : la ventilation, positive et catégorisée en dépense, rembourse
  la transaction de dépense visée. Son montant est la part remboursée ; sa catégorie, proposée d'après la
  dépense, reçoit la déduction (R-TRX-13).
- Une entrée qui rembourse plusieurs dépenses a une ventilation par dépense ; une dépense remboursée par
  plusieurs entrées reçoit plusieurs ventilations liées. Le total remboursé ne dépasse jamais la dépense.
- La déduction compte dans le mois de **l'entrée**, comme toute ventilation (R-BUD-3) : un mois clos ne change
  pas quand le remboursement arrive.
- `Transaction.expected_refund` (optionnel) : le montant attendu pour une dépense. Le reste à recevoir est
  calculé, jamais stocké (ADR-0002).
- Le lien se fait depuis l'entrée, parmi des dépenses proposées (remboursement attendu d'abord, puis même tiers,
  puis date proche).

## Conséquences

- Les montants, les budgets et les analyses ne changent pas de calcul : un remboursement reste une ventilation
  positive sur une catégorie de dépense.
- Toute écriture qui remplace les ventilations d'une transaction doit reprendre `refunds_transaction_id`, comme
  elle reprend la catégorie ou le projet.
- Supprimer la dépense retire le lien (`ON DELETE SET NULL`) ; la ventilation de l'entrée garde sa catégorie.
- Un lien ou un remboursement attendu ne change pas le solde : ils restent modifiables sur une transaction
  rapprochée (R-RAP-7).
