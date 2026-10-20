# Spec 06 — Épargne : taux déterminé et rendement variable

Statut : **Validée** (2026-10-20)
Modèle : [FixedSavingsTerms, RateSchedule, RatePeriod, Valuation](../modele-de-donnees.md#comptes--spec-01)
Décisions : ADR-0010 (épargne variable suivie par valorisations)

Les projections sont traitées dans la [spec 07](07-projections.md). Cette spec couvre le **calcul
des intérêts** (passé et présent) et la **mesure de performance**.

## Partie 1 — Épargne à taux déterminé (`FIXED_SAVINGS`)

### Concepts

Le solde d'un compte d'épargne à taux déterminé est calculé comme pour un compte courant
(R-CPT-4). Les intérêts sont des **transactions** d'origine `INTEREST`. L'application sait
aussi **calculer** les intérêts courus, ce qui sert à :
- afficher les intérêts acquis depuis le 1er janvier (non encore versés) ;
- vérifier les intérêts versés par la banque ;
- projeter (spec 07).

### Moteur d'intérêts

Le moteur est une fonction pure :
`intérêts(transactions, conditions, barème, période) → détail par sous-période + total`.

#### Méthode `FORTNIGHTLY` (quinzaines — livrets réglementés, CEL)

- **R-EPA-0** — La date d'un mouvement pour le calcul des intérêts est sa `value_date`, à défaut sa
  `bank_date`, à défaut sa `date`.
- **R-EPA-1** — L'année est découpée en 24 quinzaines : du 1er au 15, puis du 16 à la fin du mois.
- **R-EPA-2** — Un **versement** porte intérêt à partir du premier jour de la quinzaine **suivant**
  celle du versement. Exemples : versement le 3 mars → à partir du 16 mars ; versement le 16 mars →
  à partir du 1er avril ; versement le 15 mars → à partir du 16 mars.
- **R-EPA-3** — Un **retrait** cesse de porter intérêt à partir du premier jour de la quinzaine **au
  cours de laquelle** il a lieu. Exemple : retrait le 20 mars → ne porte plus intérêt dès le 16 mars.
- **R-EPA-4** — Intérêt d'une quinzaine = `solde en valeur de la quinzaine × taux annuel / 24`, où
  le taux est celui en vigueur au premier jour de la quinzaine (`RatePeriod`).
- **R-EPA-5** — Les intérêts s'accumulent sur l'année et sont **capitalisés au 31 décembre** (ou à la
  date de clôture du compte). Ils portent eux-mêmes intérêt à partir du 1er janvier suivant.
- **R-EPA-6** — Précision : les intérêts par quinzaine sont cumulés **sans arrondi** (arithmétique
  décimale exacte ou rationnelle), et le total est arrondi au centime **une seule fois**, à la
  capitalisation, au plus proche avec les demis vers le haut. Une option par compte permet d'arrondir
  chaque quinzaine si une banque procède ainsi (écart observé à l'usage).

#### Méthode `DAILY` (comptes à terme, certains livrets bancaires)

- **R-EPA-7** — Intérêt journalier = `solde de fin de journée × taux / 365` (convention ACT/365 fixe,
  y compris les années bissextiles). La date d'un mouvement suit R-EPA-0.

#### Méthode `MONTHLY`

- **R-EPA-8** — Intérêt mensuel = `solde minimum du mois × taux / 12`. Capitalisation selon les
  conditions (souvent mensuelle).

#### Communes

- **R-EPA-9** — Plafond : un versement qui porterait le **capital versé** au-delà de `deposit_cap` est
  signalé (avertissement, pas un blocage : la banque fait foi). Les intérêts capitalisés peuvent faire
  dépasser le plafond, ce qui est légal pour les livrets réglementés.
- **R-EPA-10** — À la date de capitalisation, l'application propose une transaction `INTEREST`
  `PENDING` du montant calculé, catégorisée avec `interest_category_id`. L'import du relevé la pointera
  (spec 03) ; un écart entre montant calculé et montant versé est mis en évidence.
- **R-EPA-11** — Barèmes partagés : les taux réglementés (Livret A, LDDS, LEP) sont des `RateSchedule`
  partagés. Modifier un barème partagé recalcule les intérêts de tous les comptes qui l'utilisent,
  sauf ceux déjà versés et rapprochés.
- **R-EPA-12** — Aucune valeur de taux n'est codée en dur : l'historique des taux réglementés est
  fourni en **données initiales** modifiables et doit être vérifié à la source officielle lors de sa
  saisie.

### Cas de test de référence

| Cas | Données | Attendu |
|---|---|---|
| Versement unique | 10 000 € présents au 1er janvier, taux 3 % toute l'année, aucun mouvement | 300,00 € au 31/12 |
| Versement en cours d'année | 1 000 € versés le 3 mars, 3 % | Intérêt à partir du 16 mars : 19 quinzaines × 1 000 × 0,03 / 24 = 23,75 € |
| Retrait | 2 000 € au 1er janvier, retrait de 1 000 € le 20 mars, 3 % | 5 quinzaines à 2 000 € + 19 quinzaines à 1 000 € = 12,50 + 23,75 = 36,25 € |
| Changement de taux | 10 000 € toute l'année, 3 % jusqu'au 31 janvier puis 2,4 % | 2 quinzaines à 3 % + 22 quinzaines à 2,4 % = 25,00 + 220,00 = 245,00 € |

(Taux fictifs, choisis pour des calculs ronds.)

## Partie 2 — Épargne à rendement variable (`VARIABLE_SAVINGS`)

### Concepts

On ne peut pas calculer la valeur d'une assurance vie ou d'un PEA : on la **constate** (relevé
annuel ou trimestriel, appli de l'assureur). Le suivi repose sur deux séries :
- les **apports** : transactions du compte (versements > 0, retraits < 0), en général les jambes de
  virements depuis un compte courant ;
- les **valorisations** : valeur totale du compte à une date.

La performance se déduit de ces deux séries.

### Règles

- **R-EPA-20** — Les transactions d'un compte `VARIABLE_SAVINGS` représentent **uniquement des flux
  externes** (apports et retraits). Les dividendes réinvestis, les frais prélevés dans le contrat et
  les plus-values ne sont pas saisis comme transactions : la valorisation les capture.
- **R-EPA-21** — Plus-value latente sur une période [D1, D2] = `V(D2) − V(D1) − apports nets sur ]D1, D2]`,
  où V(D1) et V(D2) sont des valorisations (ou `opening_balance` pour D1 = date d'ouverture).
- **R-EPA-22** — **TRI** (taux de rendement interne annualisé, sensible aux flux) : taux r tel que
  `Σ flux_i × (1 + r)^((D2 − d_i)/365) = V(D2)`, avec V(D1) compté comme flux initial à D1. Résolution
  par Newton avec repli sur dichotomie. Pas de résultat si la période fait moins de 30 jours (affichage « — »).
- **R-EPA-23** — **TWR** (performance pondérée par le temps, indépendante des flux) : chaînage, entre
  valorisations consécutives, de rendements de **Dietz modifié**. C'est une approximation, signalée comme telle.
- **R-EPA-24** — Performance par année civile : calculée entre les valorisations les plus proches
  du 31/12 de chaque année, à ±15 jours ; sinon « données insuffisantes ».
- **R-EPA-25** — Montant versé cumulé (« capital investi ») = `opening_balance` + Σ apports nets.
  Affiché à côté de la valeur, avec la plus-value en € et en %.

## Features

| ID | Priorité | Feature |
|---|---|---|
| EPA-01 | V1 | Barèmes de taux : création, historique daté, barèmes réglementés préremplis |
| EPA-02 | V1 | Moteur d'intérêts `FORTNIGHTLY`, avec intérêts courus de l'année en cours |
| EPA-03 | V1 | Proposition de la transaction d'intérêts annuelle et contrôle avec le montant versé |
| EPA-04 | V1 | Alerte de plafond |
| EPA-05 | V1 | Comptes variables : valorisations, capital investi, plus-value |
| EPA-06 | V1 | Performance TRI et TWR sur une période choisie, et par année civile |
| EPA-07 | V2 | Méthodes `DAILY` et `MONTHLY`, comptes à terme avec échéance |
| EPA-08 | V2 | Fiscalité indicative (net de prélèvements) paramétrable par nature de compte |
| EPA-09 | V2 | Supports au sein d'un contrat (fonds euros / UC) avec valorisations par support |

### EPA-02 — Intérêts courus

- Sur la fiche d'un Livret A, j'affiche : solde, intérêts courus depuis le 1er janvier (estimés) et
  intérêts attendus au 31/12 si aucun mouvement n'intervient d'ici là.
- Le détail par quinzaine (solde en valeur, taux, intérêt) est consultable, pour comprendre et vérifier.

### EPA-06 — Performance

- Étant donné une assurance vie ouverte avec 10 000 € le 2025-01-01, un versement de 2 000 € le
  2025-07-01 et une valorisation de 12 600 € le 2025-12-31, alors la plus-value vaut 600 €, le capital
  investi 12 000 € et le TRI ≈ 5,47 % (convention R-EPA-22 : 364 jours pour le flux initial, 183 jours
  pour le versement).
