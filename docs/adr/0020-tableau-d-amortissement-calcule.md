# ADR-0020 — Tableau d'amortissement calculé à partir des conditions du prêt

- **Statut** : Proposé
- **Date** : 2026-10-22
- **Concerne** : vision, spec 01, spec 02, spec 03, spec 07, modèle de données, glossaire, écrans, feuille de route

## Contexte

En V1, un prêt est suivi par son seul capital restant dû, saisi par valorisations (R-CPT-12), et la
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

Option 2. Un compte `LIABILITY` peut recevoir des **conditions de prêt** (`LoanTerms`) : capital
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

La **dette** reste la valeur du compte selon R-CPT-6 (solde d'ouverture, flux, valorisations) : elle
baisse par les parts de capital des mensualités payées, enregistrées comme virements vers le compte de
prêt (ADR-0021). Le tableau donne en plus le **capital restant dû théorique** à toute date, que la fiche
du prêt compare à la valeur du compte.

Hors périmètre, après la V1 : différé, remboursement anticipé, taux variable, prêt in fine, import du
tableau de la banque.

## Conséquences

- Le non-objectif « tableaux d'amortissement » de la vision est remplacé par ce périmètre restreint.
- Le domaine gagne un moteur d'amortissement pur, testé contre des tableaux de référence.
- Chaque mensualité payée peut être ventilée automatiquement en capital, intérêts et assurance
  (ADR-0021, R-CPT-17) : les dépenses ne comptent plus que les intérêts et l'assurance.
- La projection suit le tableau pour un prêt qui a des conditions (R-PRJ-40), et ses échéances futures
  alimentent la trésorerie projetée du compte payeur.
- Un prêt sans conditions garde le fonctionnement actuel (valorisations, mensualité en dépense entière).
- Nouvelle table `loan_terms` (migration), nouvelles routes d'API et écrans : conditions du prêt dans
  la fiche, tableau d'amortissement consultable.
