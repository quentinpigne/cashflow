# Glossaire

Statut : **Validée** (2026-10-20)

Langage commun du projet. Chaque terme métier a **un seul** nom français (documentation) et
**un seul** identifiant anglais (code). En cas de doute, ce fichier fait foi.

## Comptes et établissements

| Terme | Identifiant | Définition |
|---|---|---|
| Établissement | `Institution` | Banque, assureur ou courtier qui tient un ou plusieurs comptes. |
| Compte | `Account` | Tout contenant d'argent suivi par l'application : compte courant, livret, assurance vie… |
| Famille de compte | `AccountFamily` | Regroupement qui détermine le comportement d'un compte : `CASH`, `FIXED_SAVINGS`, `VARIABLE_SAVINGS`. |
| Nature de compte | `AccountKind` | Sous-type précis d'un compte (`CHECKING`, `LIVRET_A`, `PEA`…). Détermine sa famille. |
| Solde d'ouverture | `opening_balance` | Solde du compte à sa date d'ouverture dans l'application (pas forcément sa date d'ouverture réelle). |
| Solde calculé | `computed balance` | Solde d'ouverture + somme des transactions jusqu'à une date. Jamais stocké comme vérité. |
| Valorisation | `Valuation` | Valeur constatée d'un compte d'épargne variable (relevé d'assurance vie, de PEA…), à une date. |
| Compte clôturé | `closed_on` | Compte fermé à une date : aucune transaction possible après cette date. |
| Relevé de compte | `BANK_STATEMENT` | Document de ce type (PDF), rattaché à un compte pour un mois de relevé (CPT-11, R-DOC-10). |
| Mois du relevé | `Document.month` | Mois que couvre un relevé de compte : celui qui contient le plus de jours de sa période (R-IMP-28). |
| Mois sans relevé | `missing statement month` | Mois d'un compte sans relevé, depuis le premier archivé (R-CPT-21). |

## Biens et prêts

| Terme | Identifiant | Définition |
|---|---|---|
| Bien | `Asset` | Logement, véhicule… possédé ; compté à l'actif du patrimoine pour sa valeur. Ce n'est pas un compte (ADR-0019). |
| Prix d'achat | `purchase_price` | Prix du bien, conseillé apport et capital emprunté compris, hors frais. Valeur du bien sans estimation. |
| Estimation | `AssetValuation` | Valeur datée d'un bien, saisie à la main. |
| Valeur nette d'un bien | `net value` | Valeur du bien moins le capital restant dû des prêts qui le financent. |
| Prêt | `Loan` | Somme empruntée, comptée au passif du patrimoine pour son capital restant dû. Ce n'est pas un compte (ADR-0019). |
| Mode de suivi | `LoanMode` | `SCHEDULE` : avec tableau d'amortissement ; `MANUAL` : suivi simple, capital restant dû saisi. |
| Conditions de prêt | `loan terms` | Taux, durée, première échéance, assurance, compte payeur et catégories d'un prêt avec tableau (ADR-0020). |
| Tableau d'amortissement | `AmortizationSchedule` | Échéances d'un prêt calculées à partir de ses conditions : capital, intérêts, assurance et capital restant dû de chacune. Jamais stocké. |
| Échéance de prêt | `LoanInstallment` | Ligne du tableau d'amortissement. À distinguer de l'échéance prévue d'une récurrence. |
| Capital restant dû | `outstanding principal` | Ce qu'il reste à rembourser d'un prêt : selon le tableau, ou saisi en suivi simple (`LoanBalance`). |
| Échéance payée | `LoanPayment` | Lien entre une échéance de prêt et la mensualité qui l'a payée. |
| Remboursement de frais | `refund` | Ventilation d'une entrée d'argent qui rembourse, en tout ou partie, une dépense précise (`refunds_transaction_id`) : vient en déduction de la catégorie de dépense, le mois de l'entrée (R-TRX-50, ADR-0026). À ne pas confondre avec la ventilation de remboursement d'un prêt. |
| Remboursement attendu | `expected refund` | Montant qu'on attend pour une dépense (assurance maladie, note de frais) ; le **reste à recevoir** en est déduit des remboursements reçus (R-TRX-53). |
| Ventilation de remboursement | `repayment split` | Part d'une transaction rattachée à un prêt : capital remboursé. Ni revenu ni dépense (R-PRE-7). |
| Ventilation d'acquisition | `acquisition split` | Part d'une transaction rattachée à un bien : apport ou paiement du prix. Ni revenu ni dépense (R-BIE-8). |
| Rubrique activable | `feature setting` | Rubrique Biens, Prêts ou Salaires, masquée tant qu'elle n'est pas activée dans les réglages. |

## Salaires

| Terme | Identifiant | Définition |
|---|---|---|
| Emploi | `Employment` | Employeur, poste et dates d'un contrat de travail ; porte ses rémunérations et ses primes (ADR-0022). |
| Rémunération | `Salary` | Brut annuel de base et net mensuel versé d'un emploi, en vigueur à partir d'un mois d'effet. |
| Mois d'effet | `effective_month` | Premier mois où une rémunération s'applique. Futur : la rémunération est prévue. |
| Brut annuel de base | `annual_gross` | Salaire brut annuel hors primes (le 13e mois est une prime). |
| Net mensuel versé | `monthly_net` | Salaire net payé chaque mois, après prélèvement à la source. |
| Prime | `SalaryBonus` | Versement ponctuel ou annuel d'un emploi : 13e mois, prime, intéressement… Récurrente ou non. |
| Mois des augmentations | `raise_month` | Mois où la projection applique la hausse annuelle d'un emploi. |
| Versement décalé | `paid_next_month` | Le salaire du mois M est versé en M+1. |
| Revenu annuel | — | Net (ou brut) d'un emploi sur une année civile, primes comprises (R-SAL-7). |
| Écart de salaire | `salary gap` | Net reçu sur la catégorie d'un emploi − net attendu, pour un mois (R-SAL-9). |
| Fiche de paie | `PAYSLIP` | Document de ce type, rattaché à un emploi pour un mois de paie (SAL-09, R-DOC-10). |
| Mois de paie | `Document.month` | Mois travaillé que couvre une fiche de paie. |
| Mois sans fiche | `missing payslip month` | Mois d'un emploi sans fiche de paie, depuis la première archivée (R-SAL-18). |
| Taux de hausse des salaires | `salary_growth_rate` | Hausse annuelle du net projeté, fixée par scénario. |
| Taux de hausse des dépenses | `expense_growth_rate` | Hausse annuelle des dépenses projetées, fixée par scénario. |
| Capacité d'épargne | `savings capacity` | Revenus projetés − dépenses projetées − échéances de prêts, pour un mois de projection (R-PRJ-42). |
| Compte de versement | — | Compte courant qui reçoit le salaire, crédité de la capacité d'épargne en projection (R-PRJ-45). |
| Compte d'affectation du reste | `surplus_account_id` | Compte d'épargne vers lequel le scénario vire le reste de la capacité d'épargne. |

## Transactions

| Terme | Identifiant | Définition |
|---|---|---|
| Transaction | `Transaction` | Mouvement d'argent sur **un** compte, à une date, d'un montant signé (négatif = sortie). |
| Date d'opération | `date` | Date de l'opération telle que je la connais (saisie, ou date bancaire à l'import). Sert à l'affichage, aux budgets et aux rapports. |
| Date bancaire | `bank_date` | Date de l'opération selon la banque, connue à l'import ou au pointage. |
| Date de solde | `balance_date` | `bank_date` si elle est connue, sinon `date`. Sert aux soldes et au rapprochement. |
| Date d'achat | `purchase_date` | Date d'un paiement par carte, lue dans le libellé d'un relevé (« CB … 03/09/26 ») ; devient la date d'une transaction créée par import (R-IMP-26). |
| Date de valeur | `value_date` | Date de prise en compte par la banque pour les intérêts. Optionnelle. |
| Libellé bancaire | `bank_label` | Libellé brut, tel que fourni par la banque. Jamais modifié après import. |
| Libellé | `label` | Libellé lisible, choisi par l'utilisateur ou une règle. Par défaut, le libellé bancaire. |
| Ventilation | `Split` | Part d'une transaction affectée à une catégorie. Toute transaction a au moins une ventilation. |
| Catégorie | `Category` | Poste de classement d'une ventilation (« Alimentation > Restaurants »). Nature revenu ou dépense. |
| Nature de catégorie | `CategoryKind` | `INCOME` (revenu) ou `EXPENSE` (dépense). |
| Catégorie hors analyses | `excluded_from_analyses` | Catégorie dont les ventilations ne sont ni revenu ni dépense, comme un virement : exclues des budgets, analyses, projections et du non catégorisé, mais comptées dans les soldes (R-TRX-24). Celle fournie de base s'appelle « Sans catégorie » (R-TRX-25). |
| Sans catégorie | — | Catégorie fournie de base, hors analyses : l'opération n'a **volontairement** pas de catégorie. À distinguer de « non catégorisé », une opération pas encore traitée (R-TRX-12, R-TRX-25). |
| Virement interne | `Transfer` | Lien entre deux ventilations de sens opposé, de deux transactions sur deux comptes suivis (ADR-0021). Ni revenu ni dépense. |
| Ventilation de virement | `transfer split` | Ventilation, unique, d'une jambe de virement : sans catégorie, exclue des revenus, des dépenses et des budgets. |
| Jambe (de virement) | `leg` | L'une des deux transactions d'un virement interne. |
| Statut de pointage | `ClearingStatus` | `PENDING` (saisie, non vue en banque), `CLEARED` (vue sur un relevé), `RECONCILED` (couverte par un point de contrôle validé). |
| Origine | `TransactionOrigin` | `MANUAL`, `IMPORT`, `RECURRING`, `INTEREST` (intérêts générés). |
| Récurrence | `RecurringRule` | Modèle de transaction qui se répète (salaire, loyer…), source d'échéances prévues. |
| Échéance prévue | `ScheduledOccurrence` | Occurrence future calculée d'une récurrence. Ce n'est pas une transaction. |

## Tiers

| Terme | Identifiant | Définition |
|---|---|---|
| Tiers | `Payee` | Personne ou organisme de l'autre côté d'une transaction : magasin, employeur, administration, proche. Facultatif, porté par la transaction (spec 11). |
| Groupe de tiers | `PayeeGroup` | Regroupement de tiers, sur un seul niveau : l'enseigne « Carrefour » pour ses magasins, « Famille »… Jamais affecté directement à une transaction. |
| Adresse (d'un tiers) | `PayeeAddress` | Ligne d'adresse, code postal, ville et pays d'un tiers (R-TIE-23). Facultative. |
| Coordonnées | `GeoPoint` | Latitude et longitude d'une adresse, en millionièmes de degré (`latitude_e6`, `longitude_e6`), entiers. |
| Département | `Department` | Département français, déduit du code postal (R-TIE-24) ; code `38`, `2A`, `974`. |
| Lieu | `Location` | Pays, département ou ville selon lequel l'analyse par lieu regroupe les tiers (R-TIE-28). |
| Recherche d'adresse, géocodage | `Geocoding` | Proposition d'adresses et de coordonnées par la Base Adresse Nationale (BAN), à partir d'un texte saisi (R-TIE-26, ADR-0025). |

## Documents

| Terme | Identifiant | Définition |
|---|---|---|
| Bibliothèque (de documents) | — | Ensemble des documents archivés, consultable et cherchable en un seul endroit (spec 12). |
| Document | `Document` | Fichier envoyé par l'utilisateur (PDF, JPEG, PNG), immuable, conservé sur le disque (ADR-0024), avec sa description : type, titre, date, période, mois, note. |
| Type de document | `DocumentKind` | `BANK_STATEMENT`, `PAYSLIP`, `CONTRACT`, `RECEIPT`, `INVOICE`, `TAX`, `CERTIFICATE`, `LETTER`, `DEED`, `SALE_AGREEMENT`, `VEHICLE_REGISTRATION`, `DIAGNOSTIC`, `QUOTE`, `WARRANTY`, `LOAN_OFFER`, `AMORTIZATION_SCHEDULE`, `INSURANCE`, `IDENTITY`, `PROOF_OF_ADDRESS`, `OTHER` (R-DOC-5). |
| À classer | `kind` absent | État d'un document envoyé sans type (R-DOC-5, R-DOC-14). |
| Rattachement (d'un document) | `DocumentLink` | Lien entre un document et un objet qu'il concerne : compte, établissement, emploi, transaction, bien, prêt, tiers, projet (R-DOC-9). Sans effet sur les montants ; à ne pas confondre avec le rattachement d'une ventilation (R-TRX-37). |
| Justificatif | — | Document rattaché à une transaction : ticket de caisse, facture (DOC-04). |
| Date de référence | `reference date` | Date qui classe un document : sa date, à défaut son mois, à défaut le début de sa période, à défaut sa date d'envoi (R-DOC-8). |
| Contenu indexé | `content_indexed` | Texte extrait d'un PDF et indexé pour la recherche ; absent pour une image ou un scan (R-DOC-19). |

## Crédit

| Terme | Identifiant | Définition |
|---|---|---|
| Simulation (de crédit) | `LoanSimulation` | Crédit envisagé, enregistré : capital, taux, durée, assurance, frais ; sa mensualité, son coût et son TAEG se calculent (spec 13). Une offre reçue est une simulation. |
| Frais de dossier | `fees` | Frais payés une fois à la banque pour monter le crédit. |
| Frais de garantie | `guarantee_fees` | Coût de la caution ou de l'hypothèque, payé une fois. |
| Coût total du crédit | `total cost` | Intérêts + assurance + frais de dossier + frais de garantie (R-CRE-3). |
| TAEG | `apr` | Taux annuel effectif global : taux actuariel qui égale le capital net des frais aux échéances actualisées (R-CRE-4). |
| Mensualité visée | `target payment` | Mensualité, assurance comprise, d'où l'on déduit un capital ou une durée (R-CRE-5). |
| Demande de crédit | `LoanApplication` | Dossier d'un projet à financer : plan de financement, capacité, pièces, démarches (spec 13). |
| Type de projet | `ProjectKind` | `EXISTING_HOME` (ancien), `NEW_HOME` (neuf), `WORKS`, `VEHICLE`, `OTHER`. À distinguer du projet de budget (`Project`). |
| Plan de financement | `financing plan` | Prix, frais de notaire, travaux, autres frais, apport et montant à emprunter (R-CRE-11). |
| Apport | `down_payment` | Part du coût du projet payée sans emprunter. |
| Taux d'endettement | `debt ratio` | (Charges + mensualité envisagée) / revenus mensuels (R-CRE-15). |
| Reste à vivre | `remaining income` | Revenus − charges − mensualité envisagée (R-CRE-15). |
| Co-emprunteur | `co_borrower` | Second emprunteur d'une demande, dont les revenus et charges sont saisis (R-CRE-16). |
| Pièce (à fournir) | `LoanApplicationPiece` | Justificatif demandé par les banques (R-CRE-17), fourni par des documents trouvés ou attribués. |
| Démarche | `LenderRequest` | Envoi d'une demande à un établissement ou un courtier, et sa réponse (R-CRE-21). |

## Import et rapprochement

| Terme | Identifiant | Définition |
|---|---|---|
| Profil d'import | `ImportProfile` | Configuration réutilisable pour lire un format de fichier (séparateur, colonnes, format de date…). |
| Lot d'import | `ImportBatch` | Un fichier importé dans un compte, avec son statut et ses lignes. Annulable. |
| Ligne d'import | `ImportLine` | Ligne lue et normalisée d'un fichier, en attente de décision. |
| Empreinte | `fingerprint` | Hash stable d'une ligne qui sert à détecter les doublons. |
| Clé d'apprentissage | `learning_key` | Libellé bancaire normalisé, sans ses parties variables (dates, numéros, références) : deux opérations chez la même contrepartie ont la même clé. Sert aux suggestions apprises. |
| Correspondance | `match` | Association proposée entre une ligne d'import et une transaction existante. |
| Décision d'import | `ImportDecision` | Choix pour une ligne : `CREATE`, `MATCH` (pointer une existante), `SKIP`. |
| Règle de catégorisation | `CategorizationRule` | Condition sur une transaction (libellé, montant, compte) et actions associées (catégorie, libellé, tiers, virement). |
| Point de contrôle | `BalanceCheckpoint` | Solde réel d'un compte à une date, issu d'un relevé ou d'une saisie. |
| Écart | `gap` | Point de contrôle − solde calculé à la même date. Doit valoir 0. |
| Rapprochement | `reconciliation` | Validation d'un point de contrôle sans écart, qui verrouille les transactions couvertes. |

## Budgets

| Terme | Identifiant | Définition |
|---|---|---|
| Ligne de budget | `BudgetLine` | Montant prévu pour une ou plusieurs catégories (dépense ou revenu, « Voiture » : Carburant et Péage) ou un objectif d'épargne, par mois ou par an. Versionnée dans le temps. |
| Objectif d'épargne | `BudgetLine` `SAVINGS` | Montant mensuel de virements internes visé vers un compte d'épargne. |
| Surcharge | `BudgetOverride` | Montant spécifique d'une ligne pour un mois (ou une année) donné. |
| Projet | `Project` | Budget ponctuel multi-catégories (vacances, travaux), auquel on rattache explicitement des ventilations. |
| Report | `rollover` | Reliquat (positif ou négatif) d'une ligne ajouté au disponible de la période suivante. |
| Quote-part mensuelle | — | Montant d'une ligne annuelle / 12, affiché et pris en compte dans le reste à budgéter du mois. |
| Reste à budgéter | — | Revenus prévus − dépenses prévues − quotes-parts annuelles − objectifs d'épargne. |
| Hors budget | — | Dépenses catégorisées dont la catégorie (ni son parent) n'a de ligne en vigueur. |
| Inclus dans les budgets | `include_in_budget` | Drapeau d'un compte : ses ventilations alimentent les budgets. |
| Épargne de précaution | `include_in_emergency_fund` | Drapeau d'un compte : son solde compte dans les liquidités disponibles (spec 08). |

## Épargne et projections

| Terme | Identifiant | Définition |
|---|---|---|
| Barème de taux | `RateSchedule` | Historique daté des taux d'un produit à taux fixe (ex. taux du Livret A). |
| Méthode de calcul d'intérêts | `InterestMethod` | `FORTNIGHTLY` (quinzaines), `DAILY` (jours exacts), `MONTHLY`. |
| Plafond | `deposit_cap` | Montant maximal de versements autorisés sur un produit réglementé. |
| Capitalisation | `capitalization` | Ajout des intérêts acquis au capital, à une fréquence donnée (annuelle pour les livrets). |
| Apport | `contribution` | Flux net entrant (versement) ou sortant (retrait) sur un compte d'épargne variable. |
| Performance | `performance` | Rendement d'un compte variable : TRI (taux de rendement interne, pondéré par les flux) et TWR (pondéré par le temps). |
| Scénario | `ProjectionScenario` | Jeu d'hypothèses (rendements, versements, frais, horizon) pour une projection. |
| Projection | `Projection` | Série de valeurs futures calculées pour un ou plusieurs comptes selon un scénario. |

## Tableau de bord

| Terme | Identifiant | Définition |
|---|---|---|
| Patrimoine | `net_worth` | Somme des soldes et valorisations des comptes inclus dans le patrimoine. |
| Taux d'épargne | `savings_rate` | (revenus − dépenses) / revenus sur une période. |
| Épargne de précaution | `emergency_fund_months` | Liquidités disponibles / dépenses mensuelles moyennes. |

## Authentification

| Terme | Identifiant | Définition |
|---|---|---|
| Session | `Session` | Connexion d'un appareil, ouverte avec le mot de passe et portée par un cookie (ADR-0015). |
| Jeton d'accès | `AccessToken` | Secret remis à une application tierce (Foodflow) pour lire certaines routes de l'API, sans session (ADR-0023). |

## Termes à éviter

| Ne pas dire | Dire | Pourquoi |
|---|---|---|
| Type de compte | Nature (`AccountKind`) ou famille (`AccountFamily`) | « Type » ne dit pas s'il s'agit de la nature précise ou de la famille. |
| Solde courant (stocké) | Solde calculé | Le solde n'est pas stocké (ADR-0002). |
| Custom label | Libellé (`label`) | Simplification. |
| Budget (pour une catégorie) | Ligne de budget | Une catégorie classe ; une ligne de budget prévoit un montant pour elle, ou pour plusieurs. |
| Enveloppe | Ligne de budget avec report, ou projet | Le modèle « enveloppes » n'a pas été retenu (ADR-0014). |
| Bénéficiaire, marchand, destinataire | Tiers (`Payee`) | Un tiers peut aussi me verser de l'argent (employeur) : « bénéficiaire » ne couvre qu'un sens. |
| Enseigne (comme entité) | Groupe de tiers (`PayeeGroup`) | Un groupe peut réunir autre chose que des magasins. |
