# Spec 02 — Transactions, catégories, ventilation, virements, récurrences

Statut : **Validée** (2026-10-20)
Modèle : [Category, Transaction, Split, Transfer, RecurringRule](../modele-de-donnees.md#transactions--spec-02)

## Concepts

Une **transaction** est un mouvement signé sur un compte : négatif pour une sortie, positif
pour une entrée. Elle est toujours découpée en une ou plusieurs **ventilations** (`Split`),
chacune portant une part du montant et une catégorie. Une transaction « simple » a une seule
ventilation, que l'interface masque.

```
Transaction  −120,00 €  « CARREFOUR MARKET 12/03 »
 ├─ Split  −95,00 €  Alimentation > Supermarché
 ├─ Split  −15,00 €  Maison > Entretien
 └─ Split  −10,00 €  Shopping > Cadeaux
```

Un **virement interne** relie deux transactions, l'une sortante et l'autre entrante, sur deux
comptes suivis. Il n'est **ni un revenu ni une dépense** : il déplace de l'argent. Le lien porte
précisément sur une **ventilation** de chaque transaction (ADR-0021), et chaque jambe a une ventilation
unique.

Deux autres sortes de ventilations sans catégorie sont, comme un virement, ni revenu ni dépense : la
**ventilation de remboursement**, rattachée à un prêt (le capital d'une mensualité, R-PRE-7), et la
**ventilation d'acquisition**, rattachée à un bien (l'apport, R-BIE-8).
Un versement vers un compte **non suivi** (le livret d'un enfant dans une autre banque, par
exemple) n'est pas un virement interne ; c'est une transaction ordinaire, catégorisée.

## Règles

### Transactions

- **R-TRX-1** — `amount` ≠ 0. Le signe porte le sens : négatif = sortie du compte.
- **R-TRX-2** — `bank_label` est immuable une fois renseigné. `label` est librement modifiable
  et vaut `bank_label` par défaut.
- **R-TRX-3** — Une transaction `RECONCILED` ne peut être ni modifiée (montant, compte, date de solde),
  ni supprimée, sans déverrouillage explicite du point de contrôle qui la couvre (spec 04, R-RAP-6).
  Le libellé, la note, la ventilation, le tiers (spec 11), et la `date` quand une `bank_date` existe,
  restent modifiables : ils n'affectent pas le solde.
- **R-TRX-4** — Modifier le montant d'une transaction à une seule ventilation met à jour cette
  ventilation. Avec plusieurs ventilations, la modification est refusée tant que la ventilation
  n'est pas rééquilibrée (l'interface propose de reporter l'écart sur une ventilation choisie).
- **R-TRX-5** — Une transaction saisie manuellement est `PENDING`. Elle devient `CLEARED`
  lorsqu'elle est pointée par une ligne d'import (spec 03) ou pointée à la main.
- **R-TRX-6** — Supprimer une transaction issue d'un import ne supprime pas la ligne d'import :
  la ligne repasse à l'état « non traitée » et pourra être réimportée.
- **R-TRX-7** — **Trois dates.** `date` est la date que je connais : saisie à la main, ou date bancaire
  pour une transaction créée par import. Elle sert à l'affichage, aux budgets et aux rapports.
  `bank_date` est la date selon la banque, renseignée à l'import (création ou pointage, R-IMP-11).
  `balance_date` = `bank_date` si elle existe, sinon `date` : c'est la seule date utilisée pour les
  soldes et le rapprochement, afin qu'une saisie datée différemment de la banque ne crée pas d'écart
  fictif. `value_date` (date de valeur) ne sert qu'aux intérêts (spec 06).

### Ventilation

- **R-TRX-10** — Toute transaction a **au moins une** ventilation, et Σ `split.amount` =
  `transaction.amount`, au centime près (ADR-0003). Toute écriture qui violerait cet invariant
  est refusée en bloc.
- **R-TRX-11** — Une ventilation a un montant non nul. Son signe peut différer de celui de la
  transaction (ex. un achat de 100 € dont 20 € de consigne remboursée), mais l'interface le signale.
- **R-TRX-12** — Une ventilation sans catégorie est « non catégorisée ». Le tableau de bord
  remonte le total non catégorisé.
- **R-TRX-13** — La nature de la catégorie (`INCOME`/`EXPENSE`) ne contraint pas le signe : une
  ventilation positive sur une catégorie de dépense est un **remboursement** et vient en déduction
  des dépenses de cette catégorie.
- **R-TRX-14** — Aide à la saisie : ventiler par montants ou par pourcentages. Les pourcentages
  sont convertis en montants à la saisie ; l'arrondi est reporté sur la dernière ventilation pour
  que la somme tombe juste.
- **R-TRX-15** — Une ventilation peut être rattachée à **un** projet (spec 05, R-BUD-20 à R-BUD-25),
  indépendamment de sa catégorie. Dans une transaction ventilée, chaque ventilation a son propre projet :
  un ticket de supermarché peut avoir une partie « vacances » et une partie « quotidien ».

### Catégories

- **R-TRX-20** — La hiérarchie a au plus 2 niveaux (catégorie > sous-catégorie). Une sous-catégorie
  a la même nature que son parent.
- **R-TRX-21** — On peut affecter une ventilation à une catégorie parente ou à une sous-catégorie.
  Les agrégats d'une catégorie parente incluent ses sous-catégories.
- **R-TRX-22** — Une catégorie utilisée ne se supprime pas : elle s'**archive** (masquée en saisie,
  conservée dans l'historique) ou se **fusionne** dans une autre, ce qui réaffecte toutes ses
  ventilations.
- **R-TRX-23** — Un jeu de catégories par défaut est proposé au premier lancement (cf. heritage.md).

### Virements internes

- **R-TRX-30** — Un virement relie exactement une ventilation sortante (< 0) et une ventilation
  entrante (> 0), de deux transactions sur deux comptes **différents** de **même devise**, avec des
  montants **opposés** (ADR-0004, ADR-0021). Une transaction porte au plus une ventilation de virement.
- **R-TRX-31** — Les deux jambes peuvent avoir des dates différentes (exécution à J+1, J+2). Un écart
  de plus de 7 jours est signalé.
- **R-TRX-32** — Une jambe de virement a une seule ventilation, sans catégorie ni projet. Elle n'entre ni
  dans les revenus, ni dans les dépenses, ni dans les budgets, ni dans le total non catégorisé.
- **R-TRX-37** — Une ventilation a au plus un rattachement : une catégorie, un prêt (remboursement,
  R-PRE-7) ou un bien (acquisition, R-BIE-8). Les ventilations de remboursement et d'acquisition sont
  exclues des revenus, des dépenses, des budgets et du total non catégorisé, comme un virement.
- **R-TRX-33** — Créer un virement depuis la saisie crée les deux jambes d'un coup (statut `PENDING`).
  Lier deux transactions existantes (cas typique après import) remplace leurs ventilations par une
  ventilation sans catégorie, après confirmation si elles étaient catégorisées.
- **R-TRX-34** — Délier un virement conserve les deux transactions, sans catégorie.
- **R-TRX-35** — Détection : pour une transaction importée, l'application propose les transactions
  candidates de montant opposé sur un autre compte suivi, à ±5 jours (réglable).
- **R-TRX-36** — **Jambe opposée automatique.** Marquer une transaction comme virement vers (ou depuis)
  un compte suivi C, à la main, par une règle (`mark_transfer_to`) ou depuis la revue d'import :
  - si une ou plusieurs transactions candidates existent sur C (R-TRX-35), elles sont proposées et
    l'utilisateur choisit ;
  - sinon, ou si l'utilisateur le demande, l'application **crée la jambe opposée** sur C : même date,
    montant opposé, même libellé, origine `MANUAL`, ventilation unique sans catégorie. Son statut est
    `PENDING` si C est de famille `CASH` ou `FIXED_SAVINGS` (un import ultérieur de C la pointera,
    R-IMP-9, en renseignant sa `bank_date`) et `CLEARED` si C est de famille `VARIABLE_SAVINGS` (pas de
    rapprochement sur ces comptes, R-RAP-10) ;
  - la création est refusée si la date est hors de la période d'ouverture de C (R-CPT-3), ou si elle
    tombe dans une période rapprochée de C (R-RAP-7).

### Récurrences

- **R-TRX-40** — Une récurrence génère des **échéances prévues** (`ScheduledOccurrence`), calculées
  à la volée et jamais stockées comme transactions. Elles alimentent la projection de trésorerie
  (spec 07) et la détection à l'import.
- **R-TRX-41** — Pour une fréquence mensuelle ancrée sur un jour qui n'existe pas dans le mois
  (le 31 en février), l'échéance tombe le dernier jour du mois.
- **R-TRX-42** — Une échéance est « honorée » lorsqu'une transaction réelle du même compte, de
  montant égal à `amount ± amount_tolerance` et de date à ±5 jours, lui est associée. Les échéances
  passées non honorées sont signalées (« prélèvement attendu non vu »).
- **R-TRX-43** — On peut matérialiser une échéance en transaction `PENDING` (origine `RECURRING`)
  en un clic, avec la ventilation du modèle.
- **R-TRX-44** — Une récurrence avec `target_account_id` produit des virements internes (versement
  mensuel sur le livret, par exemple).

## Features

| ID | Priorité | Feature |
|---|---|---|
| TRX-01 | MVP | Saisir, modifier, supprimer une transaction |
| TRX-02 | MVP | Liste des transactions d'un compte : tri, filtres (période, catégorie, statut, montant, texte), solde progressif |
| TRX-03 | MVP | Ventiler une transaction (montants ou %) |
| TRX-04 | MVP | Gérer les catégories (2 niveaux, archivage, fusion) |
| TRX-05 | MVP | Saisir un virement interne / lier deux transactions en virement |
| TRX-06 | V1 | Catégorisation en masse (sélection multiple → catégorie) |
| TRX-07 | V1 | Récurrences : création, échéancier, échéances non honorées |
| TRX-08 | V1 | Vue transverse de toutes les transactions (tous comptes) |
| TRX-09 | — | *(supprimée : remplacée par la spec 11, TIE-01 à TIE-05)* |
| TRX-10 | V2 | Étiquettes libres (tags) transverses aux catégories (« Vacances 2026 ») |
| TRX-11 | V2 | Pièces jointes (justificatifs) |

### TRX-01 — Saisir une transaction

- Champs : compte, date (aujourd'hui par défaut), montant avec sens (dépense ou revenu), libellé,
  tiers (spec 11), catégorie, note.
- La transaction est créée `PENDING`, origine `MANUAL`, avec une ventilation unique.
- Saisie rapide accessible depuis n'importe quel écran.

### TRX-02 — Liste des transactions

- Colonne « solde après opération » calculée dans l'ordre (date, puis ordre de création).
- Les transactions `PENDING` sont visuellement distinctes ; les `RECONCILED` portent un cadenas.
- Les jambes de virement affichent le compte de contrepartie.

### TRX-03 — Ventiler

- Étant donné une transaction de −120,00 €, quand je la ventile à 50 % / 30 % / 20 %, alors les
  ventilations valent −60,00 / −36,00 / −24,00 €.
- Étant donné −100,00 € ventilés en trois tiers, alors les ventilations valent −33,33 / −33,33 / −33,34 €.
- L'enregistrement est impossible tant que le reste à ventiler n'est pas nul.

### TRX-05 — Virement interne

- Étant donné un virement de 200 € du compte courant vers le Livret A le 2026-03-05, alors le
  compte courant porte −200 € et le Livret A +200 € à cette date, et aucun des deux ne figure
  dans les revenus ou les dépenses du mois.
- Étant donné une transaction importée de −300 € sur le compte courant, quand je la marque comme
  virement vers l'assurance vie et qu'aucune transaction candidate n'existe, alors une transaction de
  +300 €, `CLEARED`, est créée sur l'assurance vie, et les deux sont liées (R-TRX-36).
