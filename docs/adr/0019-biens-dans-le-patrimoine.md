# ADR-0019 — Biens et prêts hors des comptes

- **Statut** : Accepté
- **Date** : 2026-10-22
- **Concerne** : ADR-0020, ADR-0021, spec 01, spec 02, spec 03, spec 04, spec 05, spec 07, spec 08,
  spec 09, modèle de données, glossaire, écrans

## Contexte

Le patrimoine (R-TDB-1) comptait les prêts pour leur capital restant dû, sans le bien qu'ils ont financé :
un prêt immobilier le faisait plonger, alors que l'application ne connaissait pas le logement acheté.
L'apport, sorti des comptes à l'achat, disparaissait lui aussi du patrimoine sans contrepartie.

Faire des biens et des prêts des familles de comptes (`PROPERTY`, `LIABILITY`) ne convient pas : un bien
a une date et un prix d'achat, pas une date d'ouverture ni un numéro de compte ; un prêt a un capital
emprunté, une date de déblocage et des conditions, pas de relevé. Le formulaire, la liste des comptes et
l'accueil devraient masquer ou détourner des champs, et le capital remboursé passerait par une « jambe »
sur un compte de prêt qui n'existerait que pour ce besoin.

Beaucoup d'utilisateurs n'ont ni bien ni prêt : ces notions ne doivent pas encombrer leur interface.

## Options envisagées

1. **Calculer le capital remboursé d'un prêt** et l'ajouter au patrimoine. ❌ Oublie l'apport, ne permet
   pas d'ajuster la valeur du bien, et ajoute un cas particulier à la définition du patrimoine.
2. **Des familles de comptes « Bien » et « Prêt »**. ✅ Peu de nouvelles notions. ❌ Le modèle est faux :
   ni un bien ni un prêt n'est un contenant d'argent, et chaque écran ou règle sur les comptes doit les
   exclure.
3. **Deux entités à part, `Asset` et `Loan`**, avec leurs rubriques, et des ventilations de remboursement
   et d'acquisition pour le capital des mensualités et l'apport. ✅ Chaque notion a ses champs et ses
   règles ; les comptes restent des contenants d'argent ; rubriques activables. ❌ Le patrimoine agrège
   trois sources au lieu d'une.

## Décision

Option 3 (spec 09).

- **Bien** (`Asset`) : nom, type, date et prix d'achat, estimations datées (`AssetValuation`), vente
  optionnelle. Valeur = dernière estimation, sinon prix d'achat (R-BIE-2). Prix conseillé : apport et capital
  emprunté, hors frais.
- **Prêt** (`Loan`) : nom, type, organisme prêteur, date de déblocage, capital emprunté, bien financé
  optionnel, et un **mode de suivi** : avec tableau d'amortissement (ADR-0020) ou suivi simple par capitaux
  restants dus saisis (`LoanBalance`).
- **Capital restant dû d'un prêt avec tableau** : celui du tableau à la date (R-PRE-4), que les mensualités
  soient enregistrées ou non.
- **Ventilations sans catégorie, exclues des flux comme un virement** : la part de capital d'une mensualité
  est une **ventilation de remboursement** rattachée au prêt (`Split.loan_id`, R-PRE-7) ; l'apport ou le
  paiement d'un bien est une **ventilation d'acquisition** rattachée au bien (`Split.asset_id`, R-BIE-8).
  Ni l'une ni l'autre n'est une dépense : le patrimoine ne bouge pas à l'achat, et le taux d'épargne n'est
  pas faussé.
- **Patrimoine** = Σ comptes + Σ valeurs des biens − Σ capitaux restants dus des prêts, pour ce qui est
  compté dans le patrimoine (R-TDB-1).
- **Rubriques activables** dans les réglages, désactivables seulement quand elles sont vides (R-BIE-7,
  R-PRE-12). Une nouvelle installation les a désactivées.

## Conséquences

- Les comptes n'ont plus de famille « prêt » ni « bien » : `AccountFamily` revient à `CASH`,
  `FIXED_SAVINGS` et `VARIABLE_SAVINGS`.
- Nouvelles tables (`asset`, `asset_valuation`, `loan`, `loan_balance`, `loan_payment`), colonnes
  `split.loan_id` et `split.asset_id`, export (ADR-0008) étendu. La migration convertit les comptes de prêt
  et de bien existants (base de démonstration) en prêts et en biens.
- Nouveaux écrans : rubriques Biens et Prêts, fiche d'un bien, fiche d'un prêt, interrupteurs dans les
  réglages ; l'accueil gagne des encadrés Biens et Prêts, masqués si la rubrique est désactivée.
- Une jambe de virement redevient une transaction à ventilation unique : la mensualité n'a plus besoin du
  virement porté par une ventilation (ADR-0021), qui reste le modèle des virements.
