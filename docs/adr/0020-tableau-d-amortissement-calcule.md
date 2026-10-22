# ADR-0020 — Tableau d'amortissement calculé à partir des conditions du prêt

- **Statut** : Accepté
- **Date** : 2026-10-22
- **Concerne** : vision, spec 03, spec 07, spec 09, modèle de données, glossaire, écrans, feuille de route

## Contexte

Sans tableau d'amortissement, un prêt n'est suivi que par son capital restant dû, saisi à la main, et la
mensualité est une dépense entière sur le compte qui la paie. Trois défauts en découlent :

- la part de capital de chaque mensualité est comptée comme une dépense, alors qu'elle réduit une
  dette : les dépenses sont surévaluées, l'épargne du mois et le taux d'épargne sous-évalués ;
- le capital restant dû n'est à jour qu'au rythme des saisies ;
- la projection garde le prêt constant (R-PRJ-40), si bien que le patrimoine projeté ignore les
  remboursements à venir.

Le tableau d'amortissement était un non-objectif de la V1 (vision). Il est réintégré, limité au cas
le plus courant.

## Options envisagées

1. **Tableau saisi ou importé depuis la banque.** ✅ Exact au centime. ❌ Saisie lourde (240 lignes
   pour un prêt sur 20 ans), import PDF prévu seulement en V2 (OCR).
2. **Tableau calculé à partir des conditions du prêt.** ✅ Quelques champs à saisir, calcul pur et
   testable dans le domaine, comme le moteur d'intérêts des livrets. ❌ Peut s'écarter de quelques
   centimes du tableau de la banque (arrondis), et ne couvre que les cas modélisés.
3. **Les deux.** ❌ Double travail pour un gain faible tant que l'import PDF n'existe pas.

## Décision

Option 2. Un prêt (ADR-0019) suivi « avec tableau » a des **conditions** : capital
emprunté, taux nominal annuel, durée en mois, date de la première échéance, assurance emprunteur
(aucune, montant mensuel fixe, ou taux annuel sur le capital emprunté), compte payeur, catégories des
intérêts et de l'assurance.

Le **tableau d'amortissement** en découle : il est calculé par le domaine, à la volée, et n'est jamais
stocké. Cas couvert : prêt amortissable à taux fixe et mensualités constantes.

- Mensualité hors assurance : `M = P·t / (1 − (1 + t)^−n)`, avec `t` = taux annuel / 12, arrondie au
  centime (`HALF_UP`, ADR-0001) ; `M = P / n` arrondi si le taux est nul.
- Intérêts de l'échéance k : capital restant dû après l'échéance k−1 × `t`, arrondis au centime.
  Capital de l'échéance k : `M` − intérêts. La dernière échéance solde le capital restant (sa
  mensualité s'ajuste de l'écart d'arrondi).
- Assurance : montant fixe, ou capital emprunté × taux annuel / 12 arrondi au centime, identique à
  chaque échéance.
- Dates : une échéance par mois à partir de la première, au même quantième (dernier jour du mois si
  ce quantième n'existe pas, comme R-TRX-41).

La **dette** est le capital restant dû du tableau à la date (R-PRE-4), que les mensualités soient
enregistrées ou non. Chaque mensualité payée se ventile en remboursement de capital, intérêts et
assurance (R-PRE-6) : seuls les intérêts et l'assurance sont des dépenses.

Hors périmètre, après la V1 : différé, remboursement anticipé, taux variable, prêt in fine, import du
tableau de la banque.

## Conséquences

- Le non-objectif « tableaux d'amortissement » de la vision est remplacé par ce périmètre restreint.
- Le domaine gagne un moteur d'amortissement pur, testé contre des tableaux de référence.
- Les dépenses ne comptent plus que les intérêts et l'assurance des mensualités ventilées (R-PRE-6, R-PRE-7).
- La projection suit le tableau (R-PRJ-40), et les échéances futures alimentent la trésorerie projetée du
  compte payeur (R-PRJ-10).
- Un prêt irrégulier (différé, remboursement anticipé, taux variable, in fine) se suit en mode simple.
