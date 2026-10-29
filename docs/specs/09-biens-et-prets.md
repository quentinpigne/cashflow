# Spec 09 — Biens et prêts

Statut : **Validée** (2026-10-22)
Modèle : [Asset, AssetValuation, Loan, LoanBalance, LoanPayment](../modele-de-donnees.md#biens-et-prêts--spec-09)
Décisions : ADR-0019 (biens et prêts hors des comptes), ADR-0020 (tableau d'amortissement calculé)

## Concepts

Un **bien** (logement, véhicule…) et un **prêt** ne sont pas des comptes : ils n'ont ni date d'ouverture,
ni numéro de compte, ni relevé, ni transactions. Ils ont chacun leur rubrique, leur fiche et leurs propres
caractéristiques, et entrent dans le **patrimoine** : un bien à l'actif pour sa valeur, un prêt au passif
pour son capital restant dû (R-TDB-1).

L'argent, lui, circule toujours sur les comptes : l'apport et les mensualités sont des transactions du
compte qui les paie. Une mensualité se **ventile** en remboursement de capital, intérêts et assurance : seuls
les intérêts et l'assurance sont des dépenses. L'apport se ventile en **acquisition** du bien : ce n'est pas
une dépense, et le patrimoine ne bouge pas à l'achat.

Les deux rubriques sont **facultatives** : qui n'a ni bien ni prêt les désactive dans les réglages, et
l'interface n'en montre plus rien.

## Biens

- **R-BIE-1** — Un **bien** a un nom, un type (`REAL_ESTATE` bien immobilier, `VEHICLE` véhicule,
  `OTHER` autre bien), une **date d'achat** et un **prix d'achat** positif. Prix conseillé : apport et
  capital emprunté, hors frais ; le patrimoine compte alors l'apport et le capital déjà remboursé du prêt
  qui l'a financé. Optionnels : date et prix de vente, note. Il compte dans le patrimoine par défaut
  (`include_in_net_worth`). Montants en euros.
- **R-BIE-2** — La **valeur** d'un bien à une date D est sa dernière **estimation** datée au plus tard D,
  sinon son prix d'achat. Avant la date d'achat et à partir de la date de vente, le bien ne compte plus.
  Cette valeur est conventionnelle : elle n'est jamais marquée « estimée ».
- **R-BIE-3** — Une **estimation** est une valeur positive datée, entre la date d'achat et la date de
  vente ; une seule par date.
- **R-BIE-4** — Un bien n'a pas de transactions : ni solde calculé, ni import, ni rapprochement, ni budget.
  L'achat, l'apport, les frais et les travaux sont des transactions des comptes qui les paient (R-BIE-8).
- **R-BIE-5** — Projection : un bien garde sa dernière valeur, sans plus-value ni dépréciation supposées.
- **R-BIE-6** — Supprimer un bien supprime ses estimations ; les prêts qui le finançaient perdent ce lien,
  et ses ventilations d'acquisition redeviennent non catégorisées. Ses documents restent dans la
  bibliothèque, sauf demande contraire (R-DOC-12).
- **R-BIE-8** — Une **ventilation d'acquisition** est rattachée à un bien, sans catégorie ni projet : elle
  n'entre ni dans les revenus, ni dans les dépenses, ni dans les budgets, ni dans le total non catégorisé.
  On l'emploie pour l'apport ou tout paiement qui entre dans le prix d'achat ; les frais et les travaux
  restent des dépenses, sauf choix contraire. Elle se choisit dans la ventilation d'une transaction, comme
  une catégorie (« Acquisition › Appartement »).

## Prêts

- **R-PRE-1** — Un **prêt** a un nom, un type (`MORTGAGE` prêt immobilier, `CONSUMER` prêt à la
  consommation, `OTHER` autre prêt), un organisme prêteur (établissement, optionnel), une **date de
  déblocage**, un **capital emprunté** positif et un **mode de suivi** :
  - `SCHEDULE` (**avec tableau d'amortissement**) : prêt amortissable à taux fixe et mensualités
    constantes, décrit par ses conditions (R-PRE-2) ;
  - `MANUAL` (**suivi simple**) : le capital restant dû est saisi à la main (R-PRE-5), pour un prêt
    irrégulier (prêt familial, crédit renouvelable, remboursement anticipé…).
  Optionnels : le **bien financé** (R-PRE-10), la date à laquelle il est **soldé**, une note. Il compte
  dans le patrimoine par défaut. Montants en euros.
- **R-PRE-2** — **Conditions** d'un prêt avec tableau : taux nominal annuel, durée en mois (1 à 600),
  date de la première échéance, assurance emprunteur (aucune, montant mensuel fixe, ou taux annuel sur le
  capital emprunté), **compte payeur** (compte de liquidités en euros), catégories de dépense des intérêts
  et de l'assurance (« Prêts › Intérêts » et « Prêts › Assurance emprunteur » par défaut). Différé, taux
  variable et prêt in fine sont hors périmètre : on les suit en mode simple.
- **R-PRE-3** — Le **tableau d'amortissement** est calculé à partir des conditions, jamais stocké. Avec
  `P` le capital emprunté, `n` la durée et `t` le taux annuel / 12 :
  - mensualité hors assurance `M = P·t / (1 − (1 + t)^−n)`, arrondie au centime (`HALF_UP`) ;
    `M = P / n` arrondi si le taux est nul ;
  - intérêts de l'échéance k = capital restant dû après l'échéance k−1 × `t`, arrondis au centime ;
    capital de l'échéance k = `M` − intérêts ; la **dernière** échéance solde le capital restant dû, sa
    mensualité absorbant l'écart d'arrondi ;
  - assurance de chaque échéance : le montant fixe, ou `P` × taux annuel d'assurance / 12 arrondi au
    centime ;
  - dates : une par mois à partir de la première échéance, au même quantième, ou le dernier jour du mois
    si ce quantième n'existe pas (comme R-TRX-41).
- **R-PRE-4** — Le **capital restant dû** à une date D :
  - avec tableau : le capital emprunté jusqu'à la première échéance, puis le capital restant dû après la
    dernière échéance datée au plus tard D. Il suit le tableau, que les mensualités soient enregistrées
    ou non ;
  - en suivi simple : le dernier capital restant dû saisi au plus tard D, sinon le capital emprunté.
  Avant la date de déblocage et à partir de la date où il est soldé, le prêt ne compte plus.
- **R-PRE-5** — En suivi simple, on saisit des **capitaux restants dus** datés, positifs ou nuls, une seule
  saisie par date (lus sur l'espace client ou le relevé du prêt).
- **R-PRE-6** — **Ventilation d'une mensualité** (prêt avec tableau). Un débit du compte payeur dont le
  montant égale l'échéance totale (mensualité + assurance) à **±1 €** près, daté à ±5 jours d'une échéance
  pas encore payée, est proposé comme paiement de cette échéance : depuis la fiche de la transaction, depuis
  la fiche du prêt (mensualités passées, en lot) et en fin d'import (R-IMP-22). Accepter remplace ses
  ventilations par : une **ventilation de remboursement** du capital (R-PRE-7), et deux dépenses, intérêts et
  assurance, sur les catégories des conditions (sans ventilation nulle). Un écart de montant (au plus 1 €)
  est porté par l'assurance, ou par les intérêts sans assurance. Une échéance n'est payée qu'une fois.
- **R-PRE-7** — Une **ventilation de remboursement** est rattachée à un prêt, sans catégorie ni projet :
  elle n'entre ni dans les revenus, ni dans les dépenses, ni dans les budgets, ni dans le total non
  catégorisé. En suivi simple, on peut aussi ventiler à la main une part d'un débit en remboursement du prêt
  (« Remboursement › Prêt immobilier », dans la ventilation, comme une catégorie), pour la sortir des
  dépenses ; le capital restant dû reste celui des saisies (R-PRE-4).
- **R-PRE-8** — Une mensualité ventilée selon son échéance ne change ni de montant, ni de compte, ni de
  ventilation : on **annule sa ventilation** (elle redevient une sortie non catégorisée). La supprimer libère
  l'échéance.
- **R-PRE-9** — Projection : un prêt avec tableau suit son tableau (R-PRJ-40) ; en suivi simple, il garde
  son dernier capital restant dû. La trésorerie projetée du compte payeur compte les échéances futures non
  payées (R-PRJ-10).
- **R-PRE-10** — Un prêt peut être relié au **bien qu'il finance** (plusieurs prêts pour un bien). La fiche
  du bien montre ses prêts et sa **valeur nette** (valeur − capital restant dû de ses prêts). Ce lien n'a
  aucun effet sur le patrimoine.
- **R-PRE-11** — Supprimer un prêt annule d'abord la ventilation de ses mensualités (après confirmation) ;
  ses capitaux restants dus saisis sont supprimés avec lui. Ses documents restent dans la bibliothèque, sauf
  demande contraire (R-DOC-12). La simulation dont il est issu perd ce lien (R-CRE-7).

Simuler un crédit et préparer une demande de crédit : spec 13.

## Activation

- **R-BIE-7** — La rubrique **Biens** s'active dans les réglages. Désactivée, l'interface ne montre aucun
  bien (menu, accueil, formulaires, détail du patrimoine). On ne peut la désactiver que s'il n'existe aucun
  bien. Une nouvelle installation l'a désactivée.
- **R-PRE-12** — Même règle pour la rubrique **Prêts** : désactivable seulement s'il n'existe aucun prêt ;
  désactivée, aucune proposition de ventilation de mensualité n'est faite.

## Features

| ID | Priorité | Feature |
|---|---|---|
| BIE-01 | V1 | Créer, modifier, supprimer un bien ; estimations datées ; valeur nette avec ses prêts |
| BIE-02 | V1 | Activer ou désactiver la rubrique Biens |
| BIE-03 | V1 | Ventiler l'apport en acquisition d'un bien |
| PRE-01 | V1 | Créer, modifier, solder, supprimer un prêt, en suivi simple ou avec tableau |
| PRE-02 | V1 | Tableau d'amortissement calculé (reprend CPT-09) |
| PRE-03 | V1 | Ventiler les mensualités en remboursement, intérêts et assurance (reprend CPT-10) |
| PRE-04 | V1 | Activer ou désactiver la rubrique Prêts |

### BIE-01 — Biens

- Étant donné un appartement acheté 250 000 € le 2019-06-15, sans estimation, sa valeur au 2026-10-20 est
  250 000 € ; avec une estimation de 280 000 € au 2026-01-01, elle est de 280 000 €.
- Vendu le 2026-09-01, il ne compte plus dans le patrimoine au 2026-10-20.

### BIE-03 — Acquisition

- Étant donné un débit de −50 000 € « APPORT NOTAIRE » ventilé en « Acquisition › Appartement », les dépenses
  du mois n'en comptent rien, et le patrimoine est inchangé : le compte baisse de 50 000 €, l'appartement
  vaut son prix d'achat.

### PRE-02 — Tableau d'amortissement

- Étant donné un prêt de 200 000 € à 3 % sur 240 mois, première échéance le 2026-01-05, la mensualité
  hors assurance vaut 1 109,20 € ; la première échéance compte 500,00 € d'intérêts et 609,20 € de capital.
- La somme des parts de capital de toutes les échéances égale le capital emprunté, au centime près.
- Avec une assurance à 0,30 % du capital emprunté, chaque échéance compte 50,00 € d'assurance.
- Le capital restant dû au 2026-01-04 est 200 000 € ; au 2026-01-05, 199 390,80 €.

### PRE-03 — Ventilation des mensualités

- Étant donné ce prêt et un débit de −1 159,20 € le 2026-01-05 sur le compte payeur, quand j'accepte de
  l'associer à la première échéance, alors il est ventilé en −609,20 € de remboursement, −500,00 €
  d'intérêts et −50,00 € d'assurance.
- Les dépenses du mois comptent 550,00 €, pas 1 159,20 €.

### BIE-02, PRE-04 — Activation

- Sur une nouvelle installation, ni « Biens » ni « Prêts » n'apparaissent dans le menu.
- Quand j'active « Prêts », la rubrique apparaît ; tant qu'un prêt existe, l'interrupteur ne peut pas être
  désactivé et l'interface explique pourquoi.
