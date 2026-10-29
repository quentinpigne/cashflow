# Modèle de données

Statut : **Validée** (2026-10-20)

Modèle **logique**. Sa traduction physique (SQLite, cf. `architecture.md` § SQLite) suit ADR-0009. Ce document est la source de vérité du schéma.
Les règles métier détaillées sont dans les specs ; ici ne figurent que les contraintes structurelles.

## Types logiques

| Type | Représentation | Référence |
|---|---|---|
| `Id` | UUIDv7 (triable dans le temps, généré côté application) | ADR-0011 |
| `Money` | Entier signé 64 bits, en **unités mineures** de la devise (centimes pour l'euro, décimales selon ISO 4217). La devise est celle du compte. | ADR-0001 |
| `Quantity`, `UnitPrice` | Décimal exact à précision libre, stocké en texte. Réservé au futur suivi des titres (V2) ; jamais un `Money`. | ADR-0001 |
| `Rate` | Entier en **millionièmes** (ppm) : 1,7 % = `17000`, 0,85 % = `8500` | ADR-0001 |
| `Date` | Date calendaire ISO 8601 `YYYY-MM-DD`, sans heure ni fuseau | ADR-0012 |
| `Timestamp` | Instant UTC, réservé à l'audit (`created_at`, `updated_at`…) | ADR-0012 |
| `Currency` | Code ISO 4217 sur 3 lettres | — |
| `Json` | Document JSON validé par un schéma propre à chaque champ | — |

### Correspondance physique SQLite (ADR-0017)

Toutes les tables sont `STRICT`. Les types absents de SQLite sont remplacés par des contraintes `CHECK` :

| Type logique | Colonne SQLite | Contrainte |
|---|---|---|
| `Id` | `TEXT` | `CHECK (length(col) = 36)` — UUID en minuscules avec tirets |
| `Money` | `INTEGER` | — (unités mineures) |
| `Quantity`, `UnitPrice` | `TEXT` | décimal valide, analysé par le cœur (V2) |
| `Rate` | `INTEGER` | `CHECK (col >= 0)` si le taux ne peut pas être négatif |
| `Date` | `TEXT` | `CHECK (col = date(col))` — format `YYYY-MM-DD` valide |
| Mois `YYYY-MM` | `TEXT` | `CHECK (col GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]')` et mois entre 01 et 12 |
| `Timestamp` | `TEXT` | ISO 8601 UTC, `CHECK (col LIKE '%Z')` |
| booléen | `INTEGER` | `CHECK (col IN (0, 1))` |
| enum | `TEXT` | `CHECK (col IN ('VALEUR_A', 'VALEUR_B', …))` |
| `Json` | `TEXT` | `CHECK (json_valid(col))` |

Les noms de tables sont au singulier et en `snake_case` (`account`, `transaction`, `split`…). Le mot
`transaction` étant réservé en SQL, la table est nommée `ledger_transaction`.

Toutes les entités portent `id: Id`, `created_at: Timestamp` et `updated_at: Timestamp`,
qui ne sont pas répétés ci-dessous.

## Vue d'ensemble

```mermaid
erDiagram
    Institution ||--o{ Account : "tient"
    Account ||--o| FixedSavingsTerms : "conditions (si épargne fixe)"
    Asset ||--o{ AssetValuation : "estimé"
    Loan ||--o{ LoanBalance : "capital restant dû saisi"
    Loan }o--o| Asset : "finance"
    Loan ||--o{ LoanPayment : "échéances payées"
    Transaction ||--o| LoanPayment : "paie"
    Loan ||--o{ Split : "remboursé par"
    Asset ||--o{ Split : "acquis par"
    Employment ||--o{ Salary : "rémunérations successives"
    Employment ||--o{ SalaryBonus : "primes"
    Category ||--o{ Employment : "salaire reçu sur"
    Document ||--o{ DocumentLink : "rattaché par"
    DocumentLink }o--o| Account : "compte (relevés)"
    DocumentLink }o--o| Employment : "emploi (fiches de paie)"
    DocumentLink }o--o| Transaction : "justificatif"
    DocumentLink }o--o| Asset : "bien"
    DocumentLink }o--o| Loan : "prêt"
    DocumentLink }o--o| Payee : "tiers"
    LoanApplication ||--o{ LoanSimulation : "simulations et offres"
    LoanApplication ||--o{ LenderRequest : "démarches"
    LoanApplication ||--o{ LoanApplicationPiece : "pièces attribuées"
    LenderRequest }o--o| LoanSimulation : "offre reçue"
    LenderRequest ||--o{ LenderRequestEvent : "journal"
    LoanSimulation |o--o| Loan : "transformée en"
    RateSchedule ||--o{ RatePeriod : "taux successifs"
    RateSchedule ||--o{ FixedSavingsTerms : "appliqué à"
    Account ||--o{ Valuation : "valorisé (si épargne variable)"
    Account ||--o{ Transaction : "porte"
    Transaction ||--|{ Split : "ventilée en"
    Category ||--o{ Split : "classe"
    Category ||--o{ Category : "parent de"
    PayeeGroup ||--o{ Payee : "regroupe"
    Payee ||--o{ Transaction : "tiers de"
    Payee ||--o{ RecurringRule : "tiers de"
    Split ||--o| Transfer : "jambe sortante"
    Split ||--o| Transfer : "jambe entrante"
    Account ||--o{ RecurringRule : "échéancier"
    RecurringRule ||--o{ Transaction : "a généré"
    Account ||--o{ ImportBatch : "reçoit"
    ImportProfile ||--o{ ImportBatch : "lu avec"
    ImportBatch ||--|{ ImportLine : "contient"
    ImportLine |o--o| Transaction : "a créé / a pointé"
    Account ||--o{ BalanceCheckpoint : "contrôlé par"
    ImportBatch ||--o{ BalanceCheckpoint : "fournit"
    CategorizationRule }o--o{ Category : "affecte"
    ProjectionScenario }o--o{ Account : "hypothèses par compte"
    Project ||--o{ Split : "regroupe"
    BudgetLine ||--o{ BudgetLineCategory : "porte"
    Category ||--o{ BudgetLineCategory : "budgétée par"
    Account ||--o{ BudgetLine : "objectif d'épargne vers"
    BudgetLine ||--o{ BudgetOverride : "surchargée par"
```

## Comptes — [spec 01](specs/01-comptes.md)

### Institution

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte ≤ 80 | requis, unique |
| `bank_code` | texte 5 | optionnel (code banque français) |
| `bic` | texte 8 ou 11 | optionnel |
| `archived` | booléen | défaut `false` |

### Account

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte ≤ 80 | requis |
| `kind` | `AccountKind` | requis, non modifiable si le compte a des transactions ou des valorisations |
| `institution_id` | `Id` → Institution | optionnel (ex. espèces) |
| `currency` | `Currency` | requis, défaut `EUR`, non modifiable |
| `opening_date` | `Date` | requis |
| `opening_balance` | `Money` | requis, défaut 0 |
| `iban` | texte ≤ 34 | optionnel, validé (clé de contrôle), affiché masqué |
| `account_number` | texte ≤ 30 | optionnel |
| `closed_on` | `Date` | optionnel, ≥ `opening_date` |
| `include_in_net_worth` | booléen | défaut `true` |
| `include_in_emergency_fund` | booléen | défaut `true` pour `CASH`, `LIVRET_A`, `LDDS`, `LEP`, `LIVRET_JEUNE` ; `false` sinon (R-CPT-13) |
| `include_in_budget` | booléen | défaut `true` pour `CASH`, `false` sinon ; toujours `false` pour `VARIABLE_SAVINGS` (R-BUD-1) |
| `display_order` | entier | ordre dans la famille ; à la création, après le dernier compte de la famille (CPT-07) |
| `color` | texte | optionnel, `#RRGGBB` (CPT-07) |
| `notes` | texte | optionnel |

`AccountKind` → `AccountFamily` (correspondance fixe, cf. spec 01) :

| Famille | Natures |
|---|---|
| `CASH` | `CHECKING`, `CASH_WALLET`, `OTHER_CASH` |
| `FIXED_SAVINGS` | `LIVRET_A`, `LDDS`, `LEP`, `LIVRET_JEUNE`, `CEL`, `PEL`, `TERM_DEPOSIT`, `SAVINGS_OTHER` |
| `VARIABLE_SAVINGS` | `LIFE_INSURANCE`, `PEA`, `PEA_PME`, `PER`, `SECURITIES`, `EMPLOYEE_SAVINGS`, `CRYPTO`, `VARIABLE_OTHER` |

### FixedSavingsTerms (1–1 avec un compte `FIXED_SAVINGS`)

| Champ | Type | Contraintes |
|---|---|---|
| `account_id` | `Id` → Account | clé primaire |
| `rate_schedule_id` | `Id` → RateSchedule | requis |
| `interest_method` | `InterestMethod` | `FORTNIGHTLY` \| `DAILY` \| `MONTHLY` |
| `capitalization` | enum | `ANNUAL` (au 31/12) \| `MONTHLY` \| `AT_MATURITY` |
| `deposit_cap` | `Money` | optionnel |
| `maturity_date` | `Date` | optionnel (compte à terme, PEL) |
| `interest_category_id` | `Id` → Category | catégorie des transactions d'intérêts générées |
| `round_each_fortnight` | `bool` | défaut `false` : arrondir l'intérêt de chaque quinzaine plutôt que le total (R-EPA-6) |

### RateSchedule / RatePeriod

| RateSchedule | Type | Contraintes |
|---|---|---|
| `name` | texte | requis, unique (ex. « Livret A (réglementé) ») |
| `shared` | booléen | `true` si plusieurs comptes peuvent l'utiliser (taux réglementés) |

| RatePeriod | Type | Contraintes |
|---|---|---|
| `schedule_id` | `Id` → RateSchedule | requis |
| `effective_from` | `Date` | requis, unique par barème |
| `annual_rate` | `Rate` | requis, ≥ 0 |

Un taux s'applique de `effective_from` jusqu'au `effective_from` suivant (exclu).

### Valuation (comptes `VARIABLE_SAVINGS`)

| Champ | Type | Contraintes |
|---|---|---|
| `account_id` | `Id` → Account | requis |
| `date` | `Date` | requis, unique avec `account_id` |
| `value` | `Money` | requis |
| `source` | enum | `MANUAL` \| `IMPORT` |
| `note` | texte | optionnel |

## Transactions — [spec 02](specs/02-transactions.md)

### Category

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte ≤ 60 | requis, unique au sein d'un même parent |
| `kind` | `CategoryKind` | `INCOME` \| `EXPENSE`, identique à celui du parent |
| `parent_id` | `Id` → Category | optionnel, profondeur max 2 |
| `archived` | booléen | défaut `false` |
| `excluded_from_analyses` | booléen | défaut `false` : catégorie hors analyses (R-TRX-24) |
| `display_order` | entier | — |
| `color`, `icon` | texte | optionnels |

### Transaction

| Champ | Type | Contraintes |
|---|---|---|
| `account_id` | `Id` → Account | requis |
| `date` | `Date` | requis, ≥ `opening_date` du compte, ≤ `closed_on` s'il existe. Date choisie par l'utilisateur : affichage, budgets, rapports |
| `bank_date` | `Date` | optionnel : date de l'opération selon la banque, renseignée à l'import ou au pointage ; mêmes bornes que `date` |
| `balance_date` | `Date` | **colonne générée** = `COALESCE(bank_date, date)` : date utilisée pour les soldes et le rapprochement (R-TRX-7) |
| `value_date` | `Date` | optionnel : date de valeur bancaire (intérêts) |
| `amount` | `Money` | requis, ≠ 0 |
| `bank_label` | texte ≤ 500 | optionnel (absent pour une saisie manuelle), immuable une fois renseigné |
| `label` | texte ≤ 200 | requis (défaut : `bank_label`) |
| `note` | texte | optionnel |
| `clearing_status` | `ClearingStatus` | `PENDING` \| `CLEARED` \| `RECONCILED` |
| `origin` | `TransactionOrigin` | `MANUAL` \| `IMPORT` \| `RECURRING` \| `INTEREST` |
| `recurring_rule_id` | `Id` → RecurringRule | optionnel |
| `fingerprint` | texte | optionnel, unique avec `account_id` quand renseigné |
| `payee_id` | `Id` → Payee | optionnel ; **doit être `null`** sur une jambe de virement (R-TIE-11) |
| `expected_refund` | `Money` | optionnel : **remboursement attendu** d'une dépense, > 0 et ≤ −`amount` (R-TRX-53) ; `null` si `amount` > 0 |

Index : (`account_id`, `balance_date`), (`account_id`, `date`), (`account_id`, `clearing_status`), `fingerprint`,
(`payee_id`, `date`).

### Split

| Champ | Type | Contraintes |
|---|---|---|
| `transaction_id` | `Id` → Transaction | requis, suppression en cascade |
| `amount` | `Money` | requis, ≠ 0 |
| `category_id` | `Id` → Category | optionnel (`null` = non catégorisé) ; **doit être `null`** sur une ventilation de virement |
| `note` | texte | optionnel |
| `position` | entier | ordre d'affichage |
| `project_id` | `Id` → Project | optionnel ; projet `ACTIVE` à l'affectation (R-BUD-24) ; `null` sur une ventilation de virement, de remboursement ou d'acquisition |
| `loan_id` | `Id` → Loan | optionnel : **ventilation de remboursement** (R-PRE-7) ; alors sans catégorie ni bien |
| `asset_id` | `Id` → Asset | optionnel : **ventilation d'acquisition** (R-BIE-8) ; alors sans catégorie ni prêt |
| `refunds_transaction_id` | `Id` → Transaction | optionnel : **remboursement de frais** (R-TRX-50) ; ventilation positive, sur une catégorie de dépense, visant une autre transaction de montant négatif. Lien retiré si la dépense est supprimée (R-TRX-55) |

Invariant : pour toute transaction, Σ `split.amount` = `transaction.amount` (ADR-0003). Pour toute dépense,
Σ des ventilations qui la remboursent ≤ −`amount` (R-TRX-52).

### Transfer

| Champ | Type | Contraintes |
|---|---|---|
| `outflow_split_id` | `Id` → Split | requis, unique, montant < 0 (ADR-0021) |
| `inflow_split_id` | `Id` → Split | requis, unique, montant > 0 |
| `note` | texte | optionnel |

Invariants : comptes distincts, même devise, montants des deux ventilations opposés (ADR-0004,
ADR-0021) ; une jambe de virement a une ventilation unique (R-TRX-32).

### RecurringRule

| Champ | Type | Contraintes |
|---|---|---|
| `account_id` | `Id` → Account | requis |
| `target_account_id` | `Id` → Account | optionnel : si renseigné, la récurrence est un virement |
| `label` | texte | requis |
| `amount` | `Money` | requis, ≠ 0 (montant attendu ; peut être estimé) |
| `amount_tolerance` | `Money` | défaut 0 : écart toléré pour le rapprochement avec une transaction réelle |
| `frequency` | enum | `WEEKLY` \| `MONTHLY` \| `QUARTERLY` \| `YEARLY` |
| `interval` | entier ≥ 1 | ex. 2 = tous les 2 mois |
| `anchor_date` | `Date` | première occurrence |
| `end_date` | `Date` | optionnel |
| `split_template` | `Json` | liste `{category_id, amount}` dont la somme vaut `amount` |
| `payee_id` | `Id` → Payee | optionnel ; `null` si `target_account_id` est renseigné (R-TIE-13) |
| `active` | booléen | — |

## Tiers — [spec 11](specs/11-tiers.md)

### PayeeGroup (groupe de tiers)

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte ≤ 80 | requis, unique sans distinction de casse |
| `note` | texte | optionnel |

### Payee (tiers)

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte ≤ 80 | requis, unique sans distinction de casse |
| `group_id` | `Id` → PayeeGroup | optionnel ; mis à `null` à la suppression du groupe (R-TIE-4) |
| `archived` | booléen | défaut `false` |
| `note` | texte | optionnel |
| `address_line` | texte ≤ 200 | optionnel (R-TIE-23) |
| `postal_code` | texte ≤ 10 | optionnel ; 5 chiffres si `country_code` = `FR` |
| `city` | texte ≤ 80 | requis dès qu'un champ de l'adresse est renseigné |
| `country_code` | texte 2 | ISO 3166-1 alpha-2 en majuscules ; requis avec `city` |
| `latitude_e6` | entier | optionnel, millionièmes de degré, entre −90 000 000 et 90 000 000 ; seulement avec une adresse (R-TIE-25) |
| `longitude_e6` | entier | optionnel, entre −180 000 000 et 180 000 000 ; présent si et seulement si `latitude_e6` l'est |

Index : `group_id`. Le département se déduit du code postal, il n'est pas stocké (R-TIE-24). Décision :
ADR-0025.

## Biens et prêts — [spec 09](specs/09-biens-et-prets.md)

Ni les biens ni les prêts ne sont des comptes (ADR-0019). Montants en euros.

### Asset (bien)

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte ≤ 80 | requis |
| `kind` | `AssetKind` | `REAL_ESTATE` \| `VEHICLE` \| `OTHER` |
| `purchase_date` | `Date` | requis |
| `purchase_price` | `Money` | requis, > 0 |
| `sold_on` | `Date` | optionnel, ≥ `purchase_date` |
| `sale_price` | `Money` | optionnel, ≥ 0, seulement si `sold_on` |
| `include_in_net_worth` | booléen | défaut `true` |
| `notes` | texte | optionnel |

### AssetValuation (estimation d'un bien)

| Champ | Type | Contraintes |
|---|---|---|
| `asset_id` | `Id` → Asset | requis, suppression en cascade |
| `date` | `Date` | requis, unique avec `asset_id`, entre l'achat et la vente |
| `value` | `Money` | requis, > 0 |
| `note` | texte | optionnel |

### Loan (prêt)

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte ≤ 80 | requis |
| `kind` | `LoanKind` | `MORTGAGE` \| `CONSUMER` \| `OTHER` |
| `institution_id` | `Id` → Institution | optionnel : organisme prêteur |
| `start_date` | `Date` | requis : date de déblocage |
| `principal` | `Money` | requis, > 0 : capital emprunté |
| `mode` | `LoanMode` | `SCHEDULE` (avec tableau) \| `MANUAL` (suivi simple) |
| `annual_rate` | `Rate` | requis si `SCHEDULE`, ≥ 0 : taux nominal annuel hors assurance |
| `duration_months` | entier | requis si `SCHEDULE`, 1 à 600 |
| `first_installment_date` | `Date` | requis si `SCHEDULE`, ≥ `start_date` |
| `insurance_mode` | enum | `NONE` \| `FIXED` \| `RATE_ON_PRINCIPAL` (`NONE` en suivi simple) |
| `insurance_amount` | `Money` | requis et ≥ 0 si `FIXED` |
| `insurance_rate` | `Rate` | requis et ≥ 0 si `RATE_ON_PRINCIPAL` |
| `payer_account_id` | `Id` → Account | requis si `SCHEDULE` ; compte `CASH` en euros |
| `interest_category_id` | `Id` → Category | requis si `SCHEDULE` ; catégorie de dépense |
| `insurance_category_id` | `Id` → Category | requis si une assurance existe ; catégorie de dépense |
| `asset_id` | `Id` → Asset | optionnel : bien financé (R-PRE-10) |
| `closed_on` | `Date` | optionnel : date où le prêt est soldé |
| `include_in_net_worth` | booléen | défaut `true` |
| `notes` | texte | optionnel |

Le tableau d'amortissement découle des conditions (R-PRE-3) et n'est pas stocké.

### LoanBalance (capital restant dû saisi, suivi simple)

| Champ | Type | Contraintes |
|---|---|---|
| `loan_id` | `Id` → Loan | requis, suppression en cascade |
| `date` | `Date` | requis, unique avec `loan_id` |
| `outstanding` | `Money` | requis, ≥ 0 |
| `note` | texte | optionnel |

### LoanPayment (échéance payée, R-PRE-6)

| Champ | Type | Contraintes |
|---|---|---|
| `loan_id` | `Id` → Loan | requis, suppression en cascade |
| `installment_number` | entier | requis, ≥ 1 ; unique avec `loan_id` |
| `transaction_id` | `Id` → Transaction | requis, unique : la mensualité du compte payeur, suppression en cascade |

La mensualité porte une ventilation de remboursement (le capital) et des ventilations de dépense (intérêts,
assurance).

## Crédit — [spec 13](specs/13-credit.md)

Décision : ADR-0027. Rien de ce qui suit n'entre dans le patrimoine, les budgets ou les projections.

### LoanSimulation (simulation de crédit)

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte ≤ 80 | requis |
| `kind` | `LoanKind` | `MORTGAGE` \| `CONSUMER` \| `OTHER` |
| `principal` | `Money` | requis, > 0 |
| `annual_rate` | `Rate` | requis, ≥ 0 : taux nominal annuel hors assurance |
| `duration_months` | entier | requis, 1 à 600 |
| `insurance_mode` | enum | `NONE` \| `FIXED` \| `RATE_ON_PRINCIPAL` |
| `insurance_amount` | `Money` | requis et ≥ 0 si `FIXED` |
| `insurance_rate` | `Rate` | requis et ≥ 0 si `RATE_ON_PRINCIPAL` |
| `fees` | `Money` | requis, ≥ 0 : frais de dossier |
| `guarantee_fees` | `Money` | requis, ≥ 0 : frais de garantie (caution, hypothèque) |
| `institution_id` | `Id` → Institution | optionnel : établissement qui la propose |
| `application_id` | `Id` → LoanApplication | optionnel, `SET NULL` à la suppression de la demande |
| `loan_id` | `Id` → Loan | optionnel, unique, `SET NULL` à la suppression du prêt : prêt créé (R-CRE-7) |
| `note` | texte | optionnel |

Mensualité, coût total, TAEG et tableau se calculent (R-CRE-2 à R-CRE-4), jamais stockés.

### LoanApplication (demande de crédit)

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte ≤ 80 | requis |
| `project_kind` | `ProjectKind` | `EXISTING_HOME` \| `NEW_HOME` \| `WORKS` \| `VEHICLE` \| `OTHER` |
| `status` | `ApplicationStatus` | `PREPARING` \| `SUBMITTED` \| `ACCEPTED` \| `ABANDONED` ; défaut `PREPARING` |
| `asset_id` | `Id` → Asset | optionnel, `SET NULL` : bien concerné |
| `price` | `Money` | requis, ≥ 0 |
| `notary_fees` | `Money` | requis, ≥ 0 ; estimé à la création (R-CRE-12) |
| `works` | `Money` | requis, ≥ 0 |
| `other_costs` | `Money` | requis, ≥ 0 |
| `down_payment` | `Money` | requis, ≥ 0, ≤ coût du projet |
| `other_income` | `Money` | requis, ≥ 0 : autres revenus mensuels |
| `other_charges` | `Money` | requis, ≥ 0 : autres charges mensuelles |
| `co_borrower_name` | texte ≤ 80 | optionnel |
| `co_borrower_income` | `Money` | requis, ≥ 0 ; 0 sans co-emprunteur |
| `co_borrower_charges` | `Money` | requis, ≥ 0 ; 0 sans co-emprunteur |
| `excluded_loan_ids` | `Json` | liste d'identifiants de prêts exclus des charges (R-CRE-14) ; défaut `[]` |
| `selected_simulation_id` | `Id` → LoanSimulation | optionnel, `SET NULL` : simulation retenue (R-CRE-15) |
| `note` | texte | optionnel |

Revenus, charges, taux d'endettement, reste à vivre, capacité et épargne restante se calculent (R-CRE-13 à
R-CRE-15).

### LoanApplicationPiece (pièce d'une demande)

| Champ | Type | Contraintes |
|---|---|---|
| `application_id` | `Id` → LoanApplication | requis, suppression en cascade |
| `piece` | texte | requis : code de la pièce du catalogue (R-CRE-17), ex. `IDENTITY`, `CO_BORROWER_PAYSLIPS` |
| `document_id` | `Id` → Document | optionnel, suppression en cascade : document attribué à la main |
| `not_required` | booléen | défaut `false` : pièce marquée non requise (R-CRE-18) |

Une ligne par document attribué (`document_id` renseigné, unique avec `application_id` et `piece`), ou une
ligne `not_required` par pièce. Pas de `id` propre ni d'horodatage.

### LenderRequest (démarche auprès d'un établissement)

| Champ | Type | Contraintes |
|---|---|---|
| `application_id` | `Id` → LoanApplication | requis, suppression en cascade |
| `institution_id` | `Id` → Institution | optionnel |
| `lender_name` | texte ≤ 80 | requis si pas d'établissement : banque ou courtier |
| `status` | `LenderRequestStatus` | `SENT` \| `OFFER` \| `REFUSED` \| `ACCEPTED` \| `WITHDRAWN` |
| `sent_on` | `Date` | requis |
| `answered_on` | `Date` | optionnel, ≥ `sent_on` |
| `simulation_id` | `Id` → LoanSimulation | optionnel, `SET NULL` : l'offre reçue (R-CRE-21) |
| `follow_up_on` | `Date` | optionnel : prochaine relance (R-CRE-25) |
| `note` | texte | optionnel |

### LenderRequestEvent (échange d'une démarche, R-CRE-24)

| Champ | Type | Contraintes |
|---|---|---|
| `lender_request_id` | `Id` → LenderRequest | requis, suppression en cascade |
| `date` | `Date` | requis |
| `kind` | `LenderEventKind` | `SENT` \| `FOLLOW_UP` \| `CALL` \| `MEETING` \| `ANSWER` \| `NOTE` |
| `note` | texte | optionnel |

### Réglage de la capacité d'achat

`credit.purchase_capacity` (`AppSetting`, `Json`) garde les hypothèses de la capacité d'achat (R-CRE-30) :
type de projet, taux, durée, assurance, frais, taux d'endettement visé, réserve en mois de dépenses, comptes
mobilisables, autres revenus et charges, revenus et charges du co-emprunteur, prêts exclus. Absent, les valeurs
par défaut s'appliquent. Les résultats se calculent (R-CRE-31 à R-CRE-35).

### Réglages

Les rubriques Biens et Prêts s'activent par deux réglages (`AppSetting`) : `features.assets`,
`features.loans` (R-BIE-7, R-PRE-12).

## Salaires — [spec 10](specs/10-salaires.md)

Décision : ADR-0022. Montants en euros.

### Employment (emploi)

| Champ | Type | Contraintes |
|---|---|---|
| `employer` | texte ≤ 80 | requis |
| `job_title` | texte ≤ 80 | optionnel : poste |
| `start_date` | `Date` | requis |
| `end_date` | `Date` | optionnel, ≥ `start_date` |
| `income_category_id` | `Id` → Category | requis ; catégorie de revenu (`INCOME`) comparée au net attendu (R-SAL-9) |
| `raise_month` | entier | 1 à 12, défaut 1 : mois des augmentations (R-SAL-10) |
| `paid_next_month` | booléen | défaut `false` : le salaire du mois M est versé en M+1 |
| `notes` | texte | optionnel |

### Salary (rémunération)

| Champ | Type | Contraintes |
|---|---|---|
| `employment_id` | `Id` → Employment | requis, suppression en cascade |
| `effective_month` | `Date` | requis, premier jour d'un mois, unique avec `employment_id`, entre le mois de début et le mois de fin de l'emploi |
| `annual_gross` | `Money` | requis, > 0 : brut annuel de base, hors primes |
| `monthly_net` | `Money` | requis, > 0 : net mensuel versé, après prélèvement à la source |
| `reason` | `SalaryChangeReason` | `HIRE` \| `RAISE` \| `PROMOTION` \| `TAX_RATE` \| `OTHER` |
| `note` | texte | optionnel |

Une rémunération dont `effective_month` est futur est prévue (R-SAL-4) ; rien ne la distingue en base.

### SalaryBonus (prime)

| Champ | Type | Contraintes |
|---|---|---|
| `employment_id` | `Id` → Employment | requis, suppression en cascade |
| `label` | texte ≤ 80 | requis |
| `kind` | `BonusKind` | `THIRTEENTH_MONTH` \| `BONUS` \| `PROFIT_SHARING` \| `OTHER` |
| `paid_on` | `Date` | requis, entre le début et la fin de l'emploi |
| `net_amount` | `Money` | requis, > 0 |
| `gross_amount` | `Money` | optionnel, > 0 |
| `recurring` | booléen | défaut `true` pour `THIRTEENTH_MONTH`, `false` sinon (R-SAL-11) |
| `note` | texte | optionnel |

### Réglages

`features.salaries` active la rubrique (R-SAL-14) ; `salaries.gap_tolerance` (`Rate`, défaut 5 %) fixe la
tolérance de l'écart entre attendu et reçu (R-SAL-9).

## Documents — [spec 12](specs/12-documents.md)

Décision : ADR-0024.

### Document

| Champ | Type | Contraintes |
|---|---|---|
| `kind` | `DocumentKind` | optionnel : absent, le document est **à classer** (R-DOC-5) |
| `title` | texte ≤ 120 | requis ; par défaut le nom du fichier sans extension (R-DOC-4) |
| `document_date` | `Date` | optionnel (R-DOC-6) |
| `period_start`, `period_end` | `Date` | optionnels, indépendants ; `period_start ≤ period_end` si les deux sont renseignés |
| `month` | `Date` | premier jour d'un mois ; requis si `kind` vaut `BANK_STATEMENT` ou `PAYSLIP`, absent sinon (R-DOC-7) |
| `note` | texte | optionnel |
| `file_name` | texte ≤ 255 | requis : nom d'origine du fichier |
| `media_type` | enum | `application/pdf` \| `image/jpeg` \| `image/png`, reconnu au contenu ; `application/pdf` si `kind` vaut `BANK_STATEMENT` |
| `size_bytes` | entier | requis, > 0 et ≤ 10 Mo |
| `sha256` | texte | requis, **unique** : empreinte du contenu, 64 caractères hexadécimaux (R-DOC-3) |
| `content_indexed` | booléen | `true` si du texte a été extrait du fichier et indexé (R-DOC-19) |

`DocumentKind` : `BANK_STATEMENT` (relevé de compte), `PAYSLIP` (fiche de paie), `CONTRACT` (contrat),
`RECEIPT` (ticket de caisse), `INVOICE` (facture), `TAX` (document fiscal), `CERTIFICATE` (attestation),
`LETTER` (courrier), `DEED` (acte de propriété), `SALE_AGREEMENT` (compromis de vente),
`VEHICLE_REGISTRATION` (certificat d'immatriculation), `DIAGNOSTIC` (diagnostic), `QUOTE` (devis),
`WARRANTY` (garantie), `LOAN_OFFER` (offre de prêt), `AMORTIZATION_SCHEDULE` (tableau d'amortissement),
`INSURANCE` (contrat d'assurance), `IDENTITY` (pièce d'identité), `PROOF_OF_ADDRESS` (justificatif de
domicile), `OTHER` (autre).

Le contenu est le fichier `/data/documents/<id>`, immuable. Seuls la description (`kind` à `note`) et les
rattachements se modifient. La suppression d'un document est toujours explicite : elle déplace le fichier dans
`documents/trash/`, effacé après 400 jours (R-DOC-22).

### DocumentLink (rattachement d'un document)

| Champ | Type | Contraintes |
|---|---|---|
| `document_id` | `Id` → Document | requis, suppression en cascade |
| `account_id` | `Id` → Account | optionnel, suppression en cascade |
| `institution_id` | `Id` → Institution | optionnel, suppression en cascade |
| `employment_id` | `Id` → Employment | optionnel, suppression en cascade |
| `transaction_id` | `Id` → Transaction | optionnel, suppression en cascade |
| `asset_id` | `Id` → Asset | optionnel, suppression en cascade |
| `loan_id` | `Id` → Loan | optionnel, suppression en cascade |
| `payee_id` | `Id` → Payee | optionnel, suppression en cascade |
| `project_id` | `Id` → Project | optionnel, suppression en cascade |

**Exactement une** cible est renseignée (`CHECK`). Un document est rattaché au plus une fois à un même objet
(un index unique partiel par cible : `(document_id, account_id) WHERE account_id IS NOT NULL`…). La suppression
en cascade ne retire que le rattachement, jamais le document (R-DOC-12). Pas de `created_at`/`updated_at` :
un rattachement se crée ou se supprime, il ne se modifie pas.

Règles vérifiées par le domaine, pas par le schéma (R-DOC-10) : un document `BANK_STATEMENT` a au plus un
rattachement à un compte, un document `PAYSLIP` au plus un à un emploi ; à la création ou au changement de
type, ce rattachement est requis. Le mois d'une fiche de paie est compris entre le mois de début et le mois de
fin de son emploi (R-SAL-15).

Les vues **Relevés** d'un compte (CPT-11) et **Fiches de paie** d'un emploi (SAL-09) sont les documents de ce
type rattachés à ce compte ou à cet emploi.

### Index de recherche (R-DOC-18, R-DOC-19)

Table virtuelle FTS5 `document_search` : colonne `document_id` (`UNINDEXED`), colonnes indexées `title`,
`note`, `file_name`, `content` ; tokeniseur `unicode61 remove_diacritics 2`. Le lien passe par l'identifiant et
non par le `rowid`, que `VACUUM` peut renuméroter sur une table sans clé `INTEGER PRIMARY KEY`. Tenue à jour par le service dans la même
transaction que le document. Dérivée des documents et de leurs fichiers : ni exportée, ni soumise aux
contraintes `STRICT` (une table virtuelle ne l'est pas), reconstruite par `cashflow documents reindex`.

## Import — [spec 03](specs/03-import.md)

### ImportProfile

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte | requis, unique |
| `format` | enum | `CSV` \| `XLSX` \| `OFX` \| `PDF_OCR` |
| `settings` | `Json` | schéma détaillé dans la spec 03 (encodage, séparateurs, colonnes, filtres…) |
| `default_account_id` | `Id` → Account | optionnel |

### ImportBatch

| Champ | Type | Contraintes |
|---|---|---|
| `account_id` | `Id` → Account | requis |
| `profile_id` | `Id` → ImportProfile | optionnel |
| `file_name` | texte | requis |
| `file_sha256` | texte | requis ; avertissement si déjà importé dans ce compte |
| `status` | enum | `REVIEWING` \| `COMMITTED` \| `CANCELLED` \| `REVERTED` |
| `period_start`, `period_end` | `Date` | optionnels (déduits du fichier ou des lignes) |
| `statement_opening_balance` | `Money` | optionnel (relevé) |
| `statement_closing_balance` | `Money` | optionnel (relevé) |
| `committed_at` | `Timestamp` | optionnel |

### ImportLine

| Champ | Type | Contraintes |
|---|---|---|
| `batch_id` | `Id` → ImportBatch | requis, cascade |
| `line_number` | entier | position dans le fichier source |
| `raw` | `Json` | cellules d'origine, pour la traçabilité |
| `date`, `value_date` | `Date` | `date` requise si la ligne est valide : date bancaire de l'opération |
| `purchase_date` | `Date` | optionnel : date d'achat d'un paiement par carte, lue dans le libellé (R-IMP-26) ; date de la transaction créée |
| `amount` | `Money` | requis si la ligne est valide |
| `bank_label` | texte | requis si la ligne est valide |
| `fingerprint` | texte | calculé (spec 03, R-IMP-5) |
| `errors` | `Json` | liste d'erreurs de lecture ; non vide = ligne invalide |
| `match_kind` | enum | `NONE` \| `DUPLICATE` \| `CANDIDATE` |
| `match_transaction_id` | `Id` → Transaction | optionnel |
| `match_score` | entier 0–100 | optionnel |
| `suggested` | `Json` | libellé, ventilation, virement, tiers proposés par les règles ou appris (R-IMP-13, R-TIE-17) |
| `decision` | `ImportDecision` | `CREATE` \| `MATCH` \| `SKIP` |
| `transaction_id` | `Id` → Transaction | transaction créée ou pointée, renseignée à la validation |

### CategorizationRule

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte | requis |
| `priority` | entier | plus petit = évalué en premier |
| `conditions` | `Json` | cf. spec 03 |
| `actions` | `Json` | cf. spec 03 |
| `stop_processing` | booléen | défaut `true` |
| `active` | booléen | — |
| `hit_count` | entier | statistique |

## Rapprochement — [spec 04](specs/04-rapprochement.md)

### BalanceCheckpoint

| Champ | Type | Contraintes |
|---|---|---|
| `account_id` | `Id` → Account | requis |
| `date` | `Date` | requis, unique avec `account_id` ; solde **en fin de journée** |
| `balance` | `Money` | requis |
| `source` | enum | `STATEMENT` \| `MANUAL` |
| `import_batch_id` | `Id` → ImportBatch | optionnel |
| `reconciled_at` | `Timestamp` | renseigné quand l'écart vaut 0 et que l'utilisateur valide |

## Projections — [spec 07](specs/07-projections.md)

### ProjectionScenario

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte | requis, unique (ex. « Prudent », « Médian », « Optimiste ») |
| `horizon_months` | entier | 1–600 |
| `inflation_rate` | `Rate` | optionnel (affichage en euros constants) |
| `assumptions` | `Json` | par compte ou par famille : rendement annuel, frais annuels, versement mensuel, évolution des taux… ; capacité d'épargne (R-PRJ-42 à R-PRJ-47) : `savings_capacity` (booléen, défaut `true`), `salary_growth_rate`, `expense_growth_rate` (`Rate`), `surplus_account_id` (compte d'affectation du reste, optionnel) |
| `is_default` | booléen | un seul scénario par défaut |

## Budgets — [spec 05](specs/05-budgets.md)

Décision : ADR-0014.

### BudgetLine (une version de ligne)

| Champ | Type | Contraintes |
|---|---|---|
| `kind` | enum | `EXPENSE` \| `INCOME` \| `SAVINGS` |
| `name` | texte ≤ 60 | optionnel : nom d'une ligne sur plusieurs catégories (« Voiture ») ; `null` si `SAVINGS` |
| catégories | `BudgetLineCategory` | au moins une si `EXPENSE`/`INCOME` (nature identique), aucune si `SAVINGS` (R-BUD-10) |
| `target_account_id` | `Id` → Account | requis si `SAVINGS` (famille `FIXED_SAVINGS` ou `VARIABLE_SAVINGS`), `null` sinon |
| `periodicity` | enum | `MONTHLY` \| `YEARLY` ; `SAVINGS` est toujours `MONTHLY` |
| `year_start_month` | entier 1–12 | défaut 1, utilisé si `YEARLY` |
| `amount` | `Money` | requis, > 0 (montant par période) |
| `rollover` | booléen | défaut `false` ; interdit pour `INCOME` |
| `effective_from` | mois `YYYY-MM` | requis |
| `effective_to` | mois `YYYY-MM` | optionnel, ≥ `effective_from` |
| `alert_thresholds` | `Json` | optionnel : un ou deux pourcentages ; absent : réglage global ; liste vide : aucune alerte (R-BUD-60) |
| `previous_version_id` | `Id` → BudgetLine | optionnel : chaîne des versions |

Contrainte : pour une catégorie (ou un compte cible), les intervalles [`effective_from`, `effective_to`]
de ses lignes ne se chevauchent pas (R-BUD-11).


### BudgetLineCategory

Catégories d'une version de ligne (R-BUD-10). Une nouvelle version reçoit les siennes.

| Champ | Type | Contraintes |
|---|---|---|
| `budget_line_id` | `Id` → BudgetLine | requis ; supprimé avec la version |
| `category_id` | `Id` → Category | requis ; unique par version. À une date donnée, une catégorie est dans au plus une ligne en vigueur (R-BUD-11) |

### BudgetOverride

| Champ | Type | Contraintes |
|---|---|---|
| `budget_line_id` | `Id` → BudgetLine | requis |
| `period` | texte | `YYYY-MM` (ligne mensuelle) ou `YYYY` (année budgétaire d'une ligne annuelle) ; unique avec la ligne |
| `amount` | `Money` | requis, ≥ 0 |

### Project

| Champ | Type | Contraintes |
|---|---|---|
| `name` | texte ≤ 80 | requis, unique |
| `start_date` | `Date` | requis |
| `end_date` | `Date` | optionnel, ≥ `start_date` |
| `amount` | `Money` | requis, > 0 |
| `exclude_from_period_budgets` | booléen | défaut `true` (R-BUD-21) |
| `status` | enum | `ACTIVE` \| `CLOSED` |
| `alert_thresholds` | `Json` | optionnel, comme pour `BudgetLine` |
| `note` | texte | optionnel |

## Authentification — ADR-0006, ADR-0015, ADR-0023

### AppUser

| Champ | Type | Contraintes |
|---|---|---|
| `display_name` | texte ≤ 80 | requis |
| `password_hash` | texte | requis, argon2id |

Une seule ligne en V1 (ADR-0006). Aucune donnée métier ne référence encore `AppUser`.

### Session

| Champ | Type | Contraintes |
|---|---|---|
| `user_id` | `Id` → AppUser | requis |
| `token_hash` | texte | requis, unique (le jeton en clair n'est jamais stocké) |
| `device_label` | texte | optionnel (navigateur ou appareil, pour la liste des sessions) |
| `expires_at` | `Timestamp` | requis (30 jours glissants) |
| `last_seen_at` | `Timestamp` | requis |

### AccessToken (ADR-0023)

| Champ | Type | Contraintes |
|---|---|---|
| `user_id` | `Id` → AppUser | requis |
| `name` | texte ≤ 80 | requis, unique (« foodflow ») |
| `token_hash` | texte | requis, unique (SHA-256 ; le jeton en clair n'est jamais stocké) |
| `last_used_at` | `Timestamp` | optionnel (noté au plus une fois par heure) |

Jeton d'une application tierce, en lecture seule sur une liste de routes fixée dans le code. Sans
expiration ; la révocation le supprime. Ni `set-password` ni la déconnexion de tous les appareils ne le
ferment. Pas exporté, comme les sessions.

## Notifications

### Notification (centre de notifications, ADR-0015)

| Champ | Type | Contraintes |
|---|---|---|
| `kind` | enum | `BUDGET_THRESHOLD`, `RECONCILIATION_GAP`, `MISSED_RECURRENCE`, `INTEREST_DUE`, `BACKUP_FAILED`, `LENDER_FOLLOW_UP`… |
| `title`, `body` | texte | requis |
| `link` | texte | optionnel : route de l'interface vers l'objet concerné |
| `dedupe_key` | texte | optionnel, unique ; ex. `budget:<line_id>:2027-03:80` (R-BUD-62) |
| `read_at` | `Timestamp` | optionnel |

## Réglages

### AppSetting

Paires clé/valeur typées : activation de l'OCR et fournisseur (ADR-0007), activation de la recherche
d'adresse `geocoding.enabled` (R-TIE-26, ADR-0025), tolérance de date
pour les correspondances d'import, période de référence de l'épargne de précaution, etc.
