# ADR-0021 — Virement porté par une ventilation

- **Statut** : Proposé
- **Date** : 2026-10-22
- **Concerne** : ADR-0004, ADR-0020, spec 02, spec 03, spec 05, spec 08, modèle de données, glossaire

## Contexte

L'ADR-0004 relie un virement interne à **deux transactions** entières, dont chacune a une ventilation
unique sans catégorie (R-TRX-32). Une mensualité de prêt est pourtant un débit unique sur le relevé
du compte courant, qui mélange un virement (la part de capital, qui réduit la dette) et des dépenses
(intérêts, assurance). Pour la ventiler selon le tableau d'amortissement (ADR-0020), une même
transaction doit pouvoir porter à la fois une part de virement et des parts catégorisées.

Cet ADR complète l'ADR-0004 sans le remplacer : le principe (deux transactions ordinaires reliées,
chacune sur son compte) est conservé ; seul le point d'attache du lien change.

## Options envisagées

1. **Deux transactions distinctes sur le compte payeur** (un virement du capital, une dépense pour le
   reste). ✅ Aucune règle modifiée. ❌ Ne correspond plus au relevé : l'import voit un seul débit, le
   rapprochement et le dédoublonnage s'en trouvent faussés.
2. **Un type de ventilation spécial « remboursement de capital »**, sans lien vers le prêt. ✅ Simple.
   ❌ Rien ne garantit que la dette baisse du même montant ; deuxième mécanisme parallèle aux virements.
3. **Le virement relie deux ventilations** au lieu de deux transactions. ✅ Un seul mécanisme : un
   virement ordinaire est le cas d'une transaction à ventilation unique ; une mensualité porte une
   ventilation de virement et des ventilations de dépense. ✅ L'invariant « montants opposés » se
   vérifie toujours, au niveau des ventilations. ❌ Migration de la table des virements, et les règles
   de virement se réécrivent au niveau de la ventilation.

## Décision

Option 3. `Transfer` relie une **ventilation sortante** (< 0) et une **ventilation entrante** (> 0),
sur deux comptes distincts de même devise, de montants opposés. Une ventilation de virement n'a ni
catégorie ni projet ; elle n'entre ni dans les revenus, ni dans les dépenses, ni dans les budgets, ni
dans le total non catégorisé.

Une transaction peut porter **au plus une** ventilation de virement, à côté de ventilations ordinaires.
Un virement ordinaire reste deux transactions à ventilation unique, reliées : rien ne change pour
l'utilisateur. La création d'une ventilation de virement à côté de ventilations ordinaires est
réservée en V1 aux échéances de prêt (R-CPT-17).

## Conséquences

- R-TRX-30 à R-TRX-34 se lisent au niveau de la ventilation ; la détection des virements à l'import
  (R-TRX-35) reste au niveau de la transaction entière.
- Les agrégats excluent les ventilations de virement, non plus les transactions de virement : une
  mensualité ventilée compte ses intérêts et son assurance en dépense.
- Le patrimoine reste invariant par la part de virement ; il baisse des intérêts et de l'assurance.
- Migration : la table `transfer` passe de `outflow_transaction_id` / `inflow_transaction_id` à
  `outflow_split_id` / `inflow_split_id`, selon la procédure de l'ADR-0017 ; les virements existants
  pointent sur l'unique ventilation de chaque jambe.
- L'export complet (ADR-0008) change de forme pour les virements.
