# ADR-0028 — Mois d'analyse d'une transaction

- **Statut** : Accepté
- **Date** : 2026-10-31
- **Concerne** : spec 02, spec 03, spec 05, spec 07, spec 08, spec 10, spec 11, modèle de données, écrans E7,
  E14, E16

## Contexte

Une transaction a déjà trois dates (R-TRX-7) : la `date` que je connais, la `bank_date` de la banque et la
`balance_date` qui en découle pour les soldes. La `date` sert aussi aux budgets et aux analyses. Or le mois
sur lequel une opération doit compter n'est pas toujours celui de sa date : un salaire versé le 28 fait vivre
le mois suivant, une facture payée en avance appartient au mois qu'elle couvre. Changer la `date` pour cela
fausserait l'affichage et la saisie, et la `date` d'une transaction importée vient de la banque.

Contraintes :
- les soldes, le rapprochement et les intérêts ne doivent pas bouger ;
- tous les chiffres d'un même mois doivent concorder (budget, analyses, accueil, salaires, projections) ;
- le décalage est souvent régulier (le salaire, chaque mois) : il doit pouvoir s'automatiser ;
- les requêtes d'analyse filtrent aujourd'hui sur `date` ; elles sont nombreuses.

## Options envisagées

1. **Modifier la `date`** — ✅ rien à concevoir. ❌ La date affichée devient fausse ; une transaction importée
   perd sa date réelle.
2. **Mois d'analyse par ventilation** — ✅ une transaction peut compter sur plusieurs mois. ❌ Saisie et
   affichage lourds pour un besoin qui porte presque toujours sur toute la transaction.
3. **Mois d'analyse par transaction, et une date d'analyse générée** — un `analysis_month` facultatif sur la
   transaction ; une colonne générée `analysis_date` vaut la `date` si le mois d'analyse est vide ou égal au
   mois de la date, sinon le 1er du mois d'analyse. ✅ Les requêtes d'analyse remplacent `date` par
   `analysis_date`, le domaine ne change pas. ✅ Les soldes gardent `balance_date`. ❌ Une transaction
   peut compter sur un mois futur : l'évolution doit en tenir compte.

## Décision

Option 3. Une transaction a un **mois d'analyse** facultatif (`analysis_month`, `AAAA-MM`, n'importe quel
mois). Le budget, les analyses, l'accueil, les objectifs d'épargne, les moyennes des projections, le contrôle
des salaires et l'analyse par tiers et par lieu rangent une ventilation par la **date d'analyse** de sa
transaction (`analysis_date`, colonne générée) au lieu de sa `date`. Les soldes, le rapprochement, les
intérêts, la trésorerie projetée et l'affichage gardent leurs dates.

Le mois d'analyse se modifie à la main (une transaction ou une sélection), même sur une transaction rapprochée
(il ne touche pas au solde), ou par une **règle de catégorisation** qui décale le mois d'un nombre de mois
par rapport à la date (`shift_analysis_month`).

## Conséquences

- Le domaine reste inchangé : il reçoit des dates d'analyse au lieu de dates d'opération.
- Un mois futur peut avoir du réel : l'évolution compte, pour un mois à venir, son réel plus ce qui reste à
  venir de son budget (R-TDB-14).
- Les listes ouvertes depuis le budget ou les analyses filtrent sur le mois d'analyse, pas sur la date.
- La `date` ne décide plus seule du mois d'un budget : R-BUD-3 et R-TRX-7 sont modifiées.
