# ADR-0019 — Biens (immobilier, véhicules) dans le patrimoine

- **Statut** : Accepté
- **Date** : 2026-10-22
- **Concerne** : spec 01, spec 02, spec 04, spec 05, spec 07, spec 08, modèle de données, glossaire, écrans

## Contexte

Le patrimoine (R-TDB-1) est la somme des comptes inclus, les prêts (`LIABILITY`) comptant pour leur
capital restant dû en négatif. Un prêt immobilier fait donc plonger le patrimoine (−108 000 € sur le jeu
de démonstration) alors qu'il a servi à acheter un bien, que l'application ne connaît pas : seule la
moitié de l'opération est enregistrée. L'apport, sorti des comptes au moment de l'achat, a lui aussi
disparu du patrimoine sans contrepartie.

Ce qu'on attend : que le patrimoine progresse à mesure que le prêt est remboursé, c'est-à-dire qu'il
compte la part du bien réellement acquise.

## Options envisagées

1. **Compter le capital emprunté comme un actif du prêt** (une case « compter le bien financé » sur le
   prêt). ✅ Rapide, aucune nouvelle notion. ❌ Oublie l'apport, ne permet pas d'ajuster la valeur du
   bien, et mélange dans un même compte une dette et un actif.
2. **Calculer le capital remboursé** (capital initial − capital restant dû) et l'ajouter au patrimoine.
   ✅ Donne le chiffre voulu. ❌ Même limites que l'option 1, plus un cas particulier dans la définition
   du patrimoine ; suppose de connaître le capital initial, que l'application n'a pas toujours (le solde
   d'ouverture d'un prêt est le capital restant dû à l'ouverture dans l'application).
3. **Une famille de compte « Bien »** (`PROPERTY`) : un compte par bien, valorisé comme un compte
   d'épargne variable. ✅ Le patrimoine reste « actifs − dettes », sans cas particulier ; l'apport est
   compté ; la valeur du bien est ajustable ; les projections et le détail actif / dettes fonctionnent
   sans changement de principe. ❌ Une famille de plus (modèle, migration, écrans).

## Décision

Option 3. Une nouvelle famille `PROPERTY` (« Bien ») regroupe les natures `REAL_ESTATE` (bien
immobilier), `VEHICLE` (véhicule) et `PROPERTY_OTHER` (autre bien).

- Sa valeur suit la même règle que l'épargne variable (R-CPT-6) : dernière valorisation, plus les flux
  nets enregistrés depuis ; sans valorisation, le solde d'ouverture plus les flux. Elle est positive.
- Valeur conseillée à la création : le **prix d'achat** (apport + capital emprunté, hors frais). Le
  patrimoine compte alors l'apport plus le capital déjà remboursé. Qui ne veut compter que le capital
  remboursé saisit le seul montant emprunté.
- La valeur d'un bien est conventionnelle : elle n'est **jamais marquée « estimée »**, quelle que soit
  l'ancienneté de la dernière valorisation.
- Un bien compte dans le patrimoine par défaut, jamais dans l'épargne de précaution ni dans les budgets.
- Il peut recevoir des transactions (par exemple l'apport, sous forme de virement interne depuis le
  compte courant, ou des travaux), qui suivent les mêmes règles que celles d'un compte d'épargne
  variable : pointées d'office, sans point de contrôle.
- Les projections le gardent à sa dernière valeur, constante (ni plus-value ni dépréciation supposées).
- Aucun lien n'est fait entre un bien et le prêt qui l'a financé : le calcul n'en a pas besoin. Un tel
  lien (affichage « financé par ») pourra venir après la V1.

## Conséquences

- Le patrimoine du jeu de démonstration devient positif ; ses indicateurs restent définis par R-TDB-1.
- La contrainte `CHECK` sur la nature des comptes change : la migration recrée la table des comptes
  selon la procédure de l'ADR-0017.
- Écrans : groupe « Biens » dans la liste des comptes et sur l'accueil, natures « Bien » dans le
  formulaire de compte, fiche d'un bien sur le modèle de celle d'un compte d'épargne variable
  (valorisations, sans performance).
- La performance (TRI) et le bloc « Épargne » de l'accueil restent propres à l'épargne variable : un
  bien n'y figure pas.
- Le jeu de démonstration reçoit un bien immobilier en face du prêt.
