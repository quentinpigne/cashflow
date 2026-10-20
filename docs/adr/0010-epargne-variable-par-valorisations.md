# ADR-0010 — Épargne variable suivie par valorisations

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : spec 01, spec 06, spec 07

## Contexte

Une assurance vie, un PEA ou un PER contiennent des supports (fonds euros, ETF, actions) dont la
valeur évolue. Il existe deux manières de les suivre.

## Options envisagées

1. **Suivi ligne à ligne** : titres, nombre de parts, prix de revient, cours (récupérés en ligne ou
   saisis). ✅ Précis, permet l'allocation d'actifs. ❌ Beaucoup de saisie (arbitrages, dividendes,
   frais), dépendance à une source de cours, modèle complexe.
2. **Suivi par valorisations** : apports nets (transactions) + valeur totale constatée à date.
   ✅ Simple, couvre 100 % des produits, suffit pour la performance (TRI, TWR) et les projections.
   ❌ Pas d'allocation d'actifs, granularité limitée à la fréquence des valorisations.

## Décision

Option 2 pour la V1. Le suivi par support (fonds euros / UC) est envisagé en V2 (EPA-09), et les
lignes de titres au-delà, si le besoin se confirme.

## Conséquences

- Les transactions d'un compte variable ne représentent que des flux externes (R-EPA-20).
- La valeur courante est la dernière valorisation, ajustée des apports postérieurs (R-CPT-6), et
  marquée « estimée » si la valorisation est ancienne.
- Pas de vue d'allocation d'actifs (actions / obligations / liquidités) en V1.
