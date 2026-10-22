# Spec 01 — Comptes et établissements

Statut : **Validée** (2026-10-20)
Modèle : [Institution, Account, FixedSavingsTerms](../modele-de-donnees.md#comptes--spec-01)

## Concepts

Un **compte** est tout contenant d'argent que je veux suivre. Sa **nature** (`AccountKind`)
détermine sa **famille** (`AccountFamily`), qui fixe son comportement dans l'application :

| Famille | Exemples | Solde | Catégorisation et budgets | Épargne |
|---|---|---|---|---|
| `CASH` (liquidités) | Compte courant, espèces | Calculé à partir des transactions | Oui | — |
| `FIXED_SAVINGS` (épargne à taux déterminé) | Livret A, LDDS, LEP, Livret Jeune, CEL, PEL, compte à terme | Calculé (les intérêts sont des transactions) | Non, sauf intérêts | Intérêts calculés, projection déterministe |
| `VARIABLE_SAVINGS` (épargne à rendement variable) | Assurance vie, PEA, PEA-PME, PER, compte-titres, épargne salariale, crypto | **Dernière valorisation** (cf. R-CPT-6) | Non | Valorisations, performance, projection par scénarios |
| `LIABILITY` (passif) | Prêt immobilier, prêt à la consommation | **Dernier capital restant dû**, négatif (cf. R-CPT-6, R-CPT-12) | Non | — |
| `PROPERTY` (bien) | Bien immobilier, véhicule | **Dernière valorisation** (cf. R-CPT-6, R-CPT-14) | Non | — |

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
- **R-CPT-6** — La valeur d'un compte `VARIABLE_SAVINGS`, `LIABILITY` ou `PROPERTY` à une date D est la dernière
  valorisation V (date Dv ≤ D) **plus les flux nets** (transactions du compte) enregistrés entre Dv
  (exclu) et D (inclus). Sans aucune valorisation, la valeur est `opening_balance` + flux nets. La
  valeur est marquée « estimée » lorsque la dernière valorisation date de plus de **90 jours** (seuil
  réglable), sauf pour un bien (R-CPT-14).
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
- **R-CPT-12** — **Prêts** (`LIABILITY`). Le solde d'ouverture est négatif (capital restant dû à la date
  d'ouverture dans l'application), et les valorisations sont négatives (capital restant dû lu sur le
  tableau d'amortissement ou l'espace client). **Sans conditions de prêt**, la mensualité est une
  **dépense catégorisée** sur le compte qui la paie (catégorie « Prêts »), et le compte de prêt n'a
  aucune transaction : la dette diminue par valorisations successives. **Avec des conditions de prêt**
  (R-CPT-15), chaque mensualité payée est ventilée selon le tableau d'amortissement (R-CPT-17) et la
  dette diminue par les parts de capital.
- **R-CPT-13** — Le drapeau `include_in_emergency_fund` détermine si le solde du compte compte dans les
  liquidités disponibles de l'épargne de précaution (spec 08, R-TDB-9). Prérempli à `true` pour les
  comptes courants, les espèces et les livrets réglementés disponibles à tout moment (Livret A, LDDS,
  LEP, Livret Jeune) ; modifiable ; toujours `false` pour un prêt et pour un bien.
- **R-CPT-14** — **Biens** (`PROPERTY`, ADR-0019). Un bien (logement, véhicule…) est un actif positif,
  valorisé comme un compte d'épargne variable (R-CPT-6). Valeur conseillée à la création : le **prix
  d'achat** (apport + capital emprunté, hors frais), de sorte que le patrimoine compte l'apport et le
  capital déjà remboursé du prêt qui l'a financé ; les valorisations permettent de l'ajuster. Sa valeur
  est conventionnelle et n'est jamais marquée « estimée ». Il compte dans le patrimoine par défaut,
  jamais dans l'épargne de précaution ni dans les budgets. Ses transactions éventuelles (apport par
  virement interne, travaux) suivent les règles de l'épargne variable : pointées d'office, sans point de
  contrôle (R-RAP-10). Aucun lien n'est fait avec le prêt qui l'a financé.

### Prêts : conditions et tableau d'amortissement (ADR-0020)

- **R-CPT-15** — Un compte `LIABILITY` peut avoir des **conditions de prêt** (`LoanTerms`) : capital
  emprunté, taux nominal annuel, durée en mois, date de la première échéance, assurance emprunteur
  (aucune, montant mensuel fixe, ou taux annuel sur le capital emprunté), compte payeur (famille `CASH`),
  catégories de dépense des intérêts et de l'assurance. Cas couvert : prêt amortissable à taux fixe et
  mensualités constantes. Différé, remboursement anticipé, taux variable et prêt in fine sont hors
  périmètre.
- **R-CPT-16** — Le **tableau d'amortissement** est calculé à partir des conditions, jamais stocké.
  Avec `P` le capital emprunté, `n` la durée et `t` le taux annuel / 12 :
  - mensualité hors assurance `M = P·t / (1 − (1 + t)^−n)`, arrondie au centime (`HALF_UP`) ;
    `M = P / n` arrondi si le taux est nul ;
  - intérêts de l'échéance k = capital restant dû après l'échéance k−1 × `t`, arrondis au centime ;
    capital de l'échéance k = `M` − intérêts ; la **dernière** échéance solde le capital restant dû, sa
    mensualité absorbant l'écart d'arrondi ;
  - assurance de chaque échéance : le montant fixe, ou `P` × taux annuel d'assurance / 12 arrondi au
    centime ;
  - dates : une par mois à partir de la première échéance, au même quantième, ou le dernier jour du mois
    si ce quantième n'existe pas (comme R-TRX-41).
  La fiche du prêt montre, à toute date, le **capital restant dû théorique** du tableau à côté de la
  valeur du compte (R-CPT-6) et signale un écart.
- **R-CPT-17** — **Ventilation d'une échéance.** Une transaction du compte payeur dont le montant égale
  l'échéance totale (mensualité + assurance) à **±1 €** près, datée à ±5 jours d'une échéance pas encore
  payée, est
  proposée comme paiement de cette échéance : à l'import (R-IMP-22) ou à la demande depuis la
  transaction. Accepter remplace ses ventilations par trois : une **ventilation de virement** du
  capital vers le compte de prêt (ADR-0021), dont la jambe entrante est créée sur le prêt (`CLEARED`,
  R-TRX-36), et deux dépenses, intérêts et assurance, sur les catégories des conditions (sans
  ventilation d'assurance si elle est nulle). Un écart de montant (au plus 1 €) est porté par la
  ventilation d'assurance, ou par celle des intérêts sans assurance : le capital versé reste celui du
  tableau. Une échéance n'est payée qu'une fois. On peut aussi
  ventiler ainsi, en une fois, les mensualités passées déjà enregistrées. Une mensualité ventilée et sa jambe
  sur le prêt ne changent ni de montant, ni de compte, ni de ventilation, et leur virement ne se délie pas :
  on **annule la ventilation** (la jambe est supprimée, la mensualité redevient une sortie non catégorisée).
  Supprimer la mensualité supprime sa jambe ; la jambe ne se supprime pas seule.

## Features

| ID | Priorité | Feature |
|---|---|---|
| CPT-01 | MVP | Créer, modifier, lister les comptes |
| CPT-02 | MVP | Gérer les établissements (référentiel prérempli, modifiable) |
| CPT-03 | MVP | Solde calculé à aujourd'hui et à une date donnée |
| CPT-04 | MVP | Clôturer un compte |
| CPT-05 | V1 | Conditions d'épargne fixe : barème de taux, méthode, plafond, échéance |
| CPT-06 | V1 | Saisir des valorisations pour un compte variable, un prêt ou un bien |
| CPT-07 | V1 | Ordonner et colorer les comptes, les regrouper par famille dans la vue liste |
| CPT-08 | V2 | Importer des valorisations (CSV) |
| CPT-09 | V1 | Conditions d'un prêt et tableau d'amortissement calculé (ADR-0020) |
| CPT-10 | V1 | Ventiler chaque mensualité en capital, intérêts et assurance (ADR-0021) |

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
- Pour un prêt, la valorisation saisie est le capital restant dû, en négatif (R-CPT-12).
- Pour un bien, la valorisation saisie est sa valeur, en positif (R-CPT-14).

### CPT-09 — Tableau d'amortissement

- Étant donné un prêt de 200 000 € à 3 % sur 240 mois, première échéance le 2026-01-05, la mensualité
  hors assurance vaut 1 109,20 € ; la première échéance compte 500,00 € d'intérêts et 609,20 € de capital.
- La somme des parts de capital de toutes les échéances égale le capital emprunté, au centime près.
- Avec une assurance à 0,30 % du capital emprunté, chaque échéance compte 50,00 € d'assurance.

### CPT-10 — Ventilation des mensualités

- Étant donné ce prêt et un débit de −1 159,20 € le 2026-01-05 sur le compte payeur, quand j'accepte de
  l'associer à la première échéance, alors il est ventilé en −609,20 € de virement vers le prêt,
  −500,00 € d'intérêts et −50,00 € d'assurance ; le prêt reçoit une transaction de +609,20 €.
- Les dépenses du mois comptent 550,00 €, pas 1 159,20 € ; le patrimoine baisse de 550,00 €.
