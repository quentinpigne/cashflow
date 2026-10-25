# Spec 01 — Comptes et établissements

Statut : **Validée** (2026-10-20)
Modèle : [Institution, Account, FixedSavingsTerms](../modele-de-donnees.md#comptes--spec-01)

## Concepts

Un **compte** est tout contenant d'argent que je veux suivre. Les biens et les prêts ne sont pas des
comptes : ils ont leur propre spec ([spec 09](09-biens-et-prets.md), ADR-0019). Sa **nature** (`AccountKind`)
détermine sa **famille** (`AccountFamily`), qui fixe son comportement dans l'application :

| Famille | Exemples | Solde | Catégorisation et budgets | Épargne |
|---|---|---|---|---|
| `CASH` (liquidités) | Compte courant, espèces | Calculé à partir des transactions | Oui | — |
| `FIXED_SAVINGS` (épargne à taux déterminé) | Livret A, LDDS, LEP, Livret Jeune, CEL, PEL, compte à terme | Calculé (les intérêts sont des transactions) | Non, sauf intérêts | Intérêts calculés, projection déterministe |
| `VARIABLE_SAVINGS` (épargne à rendement variable) | Assurance vie, PEA, PEA-PME, PER, compte-titres, épargne salariale, crypto | **Dernière valorisation** (cf. R-CPT-6) | Non | Valorisations, performance, projection par scénarios |

> « Taux fixe » au sens du produit : le taux est **connu à l'avance** pour chaque période,
> même s'il change dans le temps (le taux du Livret A est révisé). Ce qui compte est qu'on
> puisse **calculer** les intérêts, alors qu'en épargne variable on ne peut que les **constater**.

## Règles

- **R-CPT-1** — La famille d'un compte est entièrement déterminée par sa nature (table dans le
  modèle de données). On ne peut pas changer la nature d'un compte qui a des transactions ou des
  valorisations.
- **R-CPT-2** — La devise d'un compte est fixée à la création. Les comptes dont la devise
  n'est pas EUR sont exclus des agrégats (patrimoine, taux d'épargne) et signalés comme tels.
- **R-CPT-3** — Aucune transaction ne peut être datée (`date` comme `bank_date`) avant `opening_date`
  ni après `closed_on`.
- **R-CPT-4** — Le solde d'un compte `CASH` ou `FIXED_SAVINGS` à une date D est
  `opening_balance + Σ amount des transactions de date de solde ≤ D` (`balance_date`, R-TRX-7). Il n'est jamais stocké comme
  source de vérité (ADR-0002).
- **R-CPT-5** — Un compte ne peut être clôturé que si son solde calculé à `closed_on` vaut 0,
  ou après confirmation explicite (cas d'un compte abandonné avec un solde résiduel).
- **R-CPT-6** — La valeur d'un compte `VARIABLE_SAVINGS` à une date D est la dernière
  valorisation V (date Dv ≤ D) **plus les flux nets** (transactions du compte) enregistrés entre Dv
  (exclu) et D (inclus). Sans aucune valorisation, la valeur est `opening_balance` + flux nets. La
  valeur est marquée « estimée » lorsque la dernière valorisation date de plus de **90 jours** (seuil
  réglable).
- **R-CPT-7** — Un compte qui a des transactions ne peut pas être supprimé, seulement clôturé.
  Supprimer un compte vide supprime aussi ses conditions, valorisations et points de contrôle.
- **R-CPT-8** — Un IBAN saisi est validé (longueur par pays, clé modulo 97) et affiché masqué
  (`FR76 •••• •••• •••• •••• •••4 21`) sauf demande explicite.
- **R-CPT-9** — Un compte `FIXED_SAVINGS` a obligatoirement des `FixedSavingsTerms`. Les natures
  réglementées (`LIVRET_A`, `LDDS`, `LEP`) sont préremplies : méthode `FORTNIGHTLY`, capitalisation
  `ANNUAL`, barème partagé correspondant et plafond par défaut, ces deux derniers étant modifiables.
  Le `LIVRET_JEUNE` est prérempli de la même façon, mais avec un barème **propre au compte**, car son
  taux est fixé par chaque banque (au moins égal à celui du Livret A).
- **R-CPT-10** — Un établissement archivé n'est plus proposé à la création de compte ; ses comptes
  existants restent intacts.
- **R-CPT-11** — Le drapeau `include_in_budget` détermine si les ventilations du compte alimentent les
  budgets (spec 05, R-BUD-1).
- **R-CPT-12** — (supprimée : les prêts ne sont plus des comptes, spec 09.)
- **R-CPT-13** — Le drapeau `include_in_emergency_fund` détermine si le solde du compte compte dans les
  liquidités disponibles de l'épargne de précaution (spec 08, R-TDB-9). Prérempli à `true` pour les
  comptes courants, les espèces et les livrets réglementés disponibles à tout moment (Livret A, LDDS,
  LEP, Livret Jeune) ; modifiable.
- **R-CPT-14** — (supprimée : les biens ne sont plus des comptes, spec 09.)
- **R-CPT-15** — (supprimée : remplacée par R-PRE-2.)
- **R-CPT-16** — (supprimée : remplacée par R-PRE-3 et R-PRE-4.)
- **R-CPT-17** — (supprimée : remplacée par R-PRE-6 à R-PRE-8.)

## Features

| ID | Priorité | Feature |
|---|---|---|
| CPT-01 | MVP | Créer, modifier, lister les comptes |
| CPT-02 | MVP | Gérer les établissements (référentiel prérempli, modifiable) |
| CPT-03 | MVP | Solde calculé à aujourd'hui et à une date donnée |
| CPT-04 | MVP | Clôturer un compte |
| CPT-05 | V1 | Conditions d'épargne fixe : barème de taux, méthode, plafond, échéance |
| CPT-06 | V1 | Saisir des valorisations pour un compte variable |
| CPT-07 | V1 | Ordonner et colorer les comptes, les regrouper par famille dans la vue liste |
| CPT-08 | V2 | Importer des valorisations (CSV) |
| CPT-09 | — | (supprimée : devenue PRE-02, spec 09) |
| CPT-10 | — | (supprimée : devenue PRE-03, spec 09) |

### CPT-01 — Créer un compte

En tant qu'utilisateur, je crée un compte en indiquant au minimum un nom, une nature, une date
d'ouverture et un solde d'ouverture.

Critères d'acceptation :
- Étant donné un compte courant créé le 2026-01-01 avec un solde d'ouverture de 1 369,93 €,
  quand je consulte son solde au 2026-01-01, alors il vaut 1 369,93 €.
- Quand je crée un `LIVRET_A`, alors ses conditions d'épargne sont préremplies (R-CPT-9).
- Quand je saisis un IBAN invalide, alors la création est refusée avec un message explicite.
- Les coordonnées bancaires (IBAN, numéro) sont toutes **optionnelles**.

### CPT-03 — Solde calculé

- Étant donné des transactions de −50 € le 2026-03-10 et de +2 000 € le 2026-03-31, le solde au
  2026-03-15 inclut −50 € et exclut +2 000 €.
- Le solde affiché distingue « solde total » (toutes transactions) et « solde pointé »
  (transactions `CLEARED` ou `RECONCILED` seulement), comme sur un relevé.

### CPT-04 — Clôturer un compte

- Clôturer un compte au solde non nul demande une confirmation (R-CPT-5).
- Un compte clôturé n'apparaît plus dans les listes de saisie, mais reste consultable et
  continue de compter dans l'historique du patrimoine jusqu'à sa date de clôture.

### CPT-06 — Valorisations

- Je saisis une valeur à une date ; une seule valorisation par date et par compte (la saisie
  remplace la précédente après confirmation).
- La valeur du compte suit R-CPT-6 ; le tableau de bord affiche « estimée » si la valorisation
  est ancienne.

### CPT-07 — Ordre et couleur des comptes

- Je choisis une couleur pour un compte parmi une palette, ou aucune ; elle apparaît en pastille devant
  son nom dans la liste des comptes et sur sa fiche.
- Dans la liste, un mode « Ordonner » me laisse glisser les comptes d'une même famille pour fixer leur
  ordre ; il est gardé (`display_order`). Les familles restent dans l'ordre de la spec.
- Un nouveau compte se range à la fin de sa famille.
