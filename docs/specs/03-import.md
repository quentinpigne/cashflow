# Spec 03 — Import de relevés et règles de catégorisation

Statut : **Validée** (2026-10-20)
Modèle : [ImportProfile, ImportBatch, ImportLine, CategorizationRule](../modele-de-donnees.md#import--spec-03)
Décisions : ADR-0005 (import en deux temps), ADR-0007 (OCR externe optionnel)

## Objectif

Importer les transactions d'un fichier bancaire dans un compte pour :
1. **créer** les transactions qui n'existent pas encore ;
2. **vérifier** les transactions déjà saisies à la main (les passer de `PENDING` à `CLEARED`) ;
3. **ne jamais dupliquer** une transaction déjà importée ;
4. **contrôler le solde** quand le relevé fournit un solde de début ou de fin (spec 04).

## Pipeline

```
 Fichier ──► 1. Lecture ──► 2. Normalisation ──► 3. Correspondances ──► 4. Suggestions ──► 5. Revue ──► 6. Validation
             (format +      (ImportLine :         (doublons,             (règles de         (utilisateur)  (atomique)
              profil)        date, montant,        candidats PENDING,     catégorisation,
                             libellé, erreurs)     virements)             virements)
```

Les étapes 1 à 4 créent un `ImportBatch` au statut `REVIEWING` et ses `ImportLine`. **Aucune
transaction n'est créée ou modifiée avant l'étape 6** (R-IMP-1).

## Formats

| Format | Priorité | Lecture |
|---|---|---|
| CSV | MVP | Profil d'import (mapping de colonnes personnalisé) |
| XLSX | MVP | Profil d'import + choix de la feuille |
| OFX / QFX | V1 | Standard : pas de mapping. `FITID` est utilisé comme empreinte. Soldes `LEDGERBAL` récupérés. |
| PDF (parseur dédié à une banque) | V1 | Parseur spécifique (LCL d'abord, cf. heritage.md) sur le texte extrait **localement** avec sa position ; OCR (opt-in) en repli pour les PDF scannés (ADR-0007) |
| PDF (générique) | V2 | OCR + extraction structurée par un modèle ; **opt-in** (ADR-0007) |

Tous les lecteurs produisent la même sortie : une liste de lignes brutes normalisées, plus des
métadonnées optionnelles de relevé (période, solde de début, solde de fin). Ajouter un format
revient à ajouter un lecteur ; le reste du pipeline ne change pas.

## Profil d'import (CSV/XLSX)

Un profil est créé la première fois via un **assistant avec aperçu**, puis réutilisé. Schéma de
`ImportProfile.settings` :

```jsonc
{
  "encoding": "windows-1252",        // auto-détecté, modifiable (UTF-8, UTF-8-BOM, ISO-8859-1, windows-1252)
  "delimiter": ";",                  // auto-détecté : ; , \t |
  "quote": "\"",
  "sheet": "Opérations",             // XLSX uniquement
  "header_row": 3,                   // ligne (1-based) des en-têtes ; null = pas d'en-tête (colonnes par index)
  "skip_rows_after_header": 0,
  "skip_footer_rows": 2,
  "date": { "column": "Date", "format": "dd/MM/yyyy" },
  "value_date": { "column": "Date valeur", "format": "dd/MM/yyyy" },     // optionnel
  "label": { "columns": ["Libellé", "Détail"], "separator": " " },       // concaténation
  "amount": {
    "mode": "single",                // "single" | "debit_credit"
    "column": "Montant",             // mode single
    "debit_column": null,            // mode debit_credit
    "credit_column": null,
    "debit_is_positive": true,       // mode debit_credit : le débit est-il écrit en positif ?
    "invert_sign": false,            // mode single : inverser le signe (export carte, par ex.)
    "decimal_separator": ",",
    "thousands_separator": " "       // espace, espace insécable, point, apostrophe, aucun
  },
  "row_filters": [                   // lignes ignorées (solde, totaux…)
    { "column": "Libellé", "pattern": "^(ANCIEN SOLDE|TOTAUX|SOLDE)", "regex": true }
  ],
  "statement_balances": {            // optionnel : où lire les soldes du relevé
    "opening": { "cell": "B2" },
    "closing": { "cell": "B3" }
  },
  "category_column": null            // optionnel : catégorie fournie par la banque → suggestion
}
```

## Règles

### Lecture et normalisation

- **R-IMP-1** — Un import ne modifie aucune donnée existante avant la validation (étape 6). Annuler
  une revue supprime le lot et ses lignes, sans autre effet.
- **R-IMP-2** — Les montants sont convertis en centimes **sans passer par un flottant**, à partir
  de la chaîne de caractères : « 1 234,56 » → 123456 ; « -0,5 » → −50 ; « (12,00) » → −1200.
  Un montant illisible ou avec plus de 2 décimales produit une erreur de ligne.
- **R-IMP-3** — Une ligne en erreur (date ou montant illisible, libellé vide) est conservée avec ses
  erreurs, affichée, et exclue de la validation. Elle ne bloque pas le reste du lot.
- **R-IMP-4** — Une date à deux chiffres d'année (« 12/03/26 ») est interprétée dans le siècle qui
  la rapproche le plus de la date du jour. Une date sans année (relevés PDF, « 12.03 ») prend l'année
  de la période du relevé ; pour un relevé à cheval sur deux années (décembre → janvier), le mois
  détermine l'année.

### Empreinte et doublons

- **R-IMP-5** — L'empreinte d'une ligne vaut
  `sha256(account_id | date | amount | normalize(bank_label) | occurrence_index)`, où :
  - `normalize` : majuscules, suppression des accents, espaces multiples réduits à un seul, bords rognés ;
  - `occurrence_index` : rang (0, 1, 2…) de la ligne parmi celles du **même fichier** qui ont la même
    date, le même montant et le même libellé normalisé. Deux cafés identiques le même jour ont ainsi
    deux empreintes distinctes, et réimporter le fichier redonne les mêmes empreintes.
  - Pour l'OFX, l'empreinte est `sha256(account_id | FITID)`.
- **R-IMP-6** — Une ligne dont l'empreinte existe déjà sur une transaction du compte est marquée
  `DUPLICATE` et sa décision par défaut est `SKIP`.
- **R-IMP-7** — Un fichier dont le `sha256` correspond à un lot déjà validé dans ce compte déclenche
  un avertissement avant lecture. Ce n'est pas bloquant : R-IMP-6 suffit à éviter les doublons.
- **R-IMP-8** — Chevauchement de relevés : deux exports successifs qui se recouvrent sur quelques
  jours ne créent pas de doublons tant que la banque fournit des libellés stables. Si la banque
  modifie ses libellés entre deux exports (cas connu : le libellé de carte « en cours » diffère du
  libellé définitif), R-IMP-9 propose un candidat et la décision revient à l'utilisateur.

### Correspondance avec les transactions existantes

- **R-IMP-9** — Pour chaque ligne non `DUPLICATE`, on cherche une transaction **candidate** du même
  compte :
  - statut `PENDING`, ou `CLEARED` sans empreinte (saisie manuelle pointée à la main) ;
  - même montant exact ;
  - date à ±5 jours (réglable dans `AppSetting`) ;
  - pas déjà associée à une autre ligne du lot.

  Score (0–100) : 60 pour le montant, + jusqu'à 25 pour la proximité de date (25 à J, −5 par jour
  d'écart), + jusqu'à 15 pour la similarité des libellés (ratio de tokens communs). Le candidat de
  meilleur score est proposé. Avec un score ≥ 80, la décision par défaut est `MATCH`, sinon `CREATE`
  avec le candidat affiché.
- **R-IMP-10** — Une échéance prévue non honorée (R-TRX-42) qui correspond à la ligne est proposée
  comme candidate ; la valider honore l'échéance.
- **R-IMP-11** — Valider un `MATCH` : la transaction existante devient `CLEARED` et reçoit
  `bank_label`, `fingerprint`, `value_date` et **`bank_date`** de la ligne. Sa `date`, son libellé, sa
  note et sa ventilation sont **conservés** (R-TRX-7). Une transaction **créée** par import a
  `date` = `bank_date` = date de la ligne.

### Suggestions

- **R-IMP-12** — Les règles de catégorisation actives sont évaluées par `priority` croissante sur chaque
  ligne `CREATE`. La première règle qui correspond s'applique ; si son `stop_processing` vaut `false`,
  les suivantes sont aussi évaluées, et les actions déjà appliquées ne sont pas écrasées.
- **R-IMP-13** — Sans règle applicable, la catégorie suggérée est celle de la dernière transaction du
  même compte au libellé normalisé identique, si elle existe (« apprentissage » minimal).
- **R-IMP-14** — Les candidats de virement interne (R-TRX-35), y compris parmi les lignes d'**autres
  lots en revue**, sont suggérés. Accepter la suggestion crée le virement à la validation. Sans
  candidat, marquer la ligne comme virement vers un compte crée la jambe opposée à la validation
  (R-TRX-36).
- **R-IMP-22** — Après la validation, les transactions du lot (créées ou pointées) qui correspondent à une
  échéance de prêt pas encore payée (R-CPT-17) sont proposées sur l'écran de fin d'import, comme le
  rapprochement (R-RAP-11). Accepter les ventile en capital, intérêts et assurance et crée les jambes sur le
  prêt. Rien n'est ventilé sans cet accord.

### Validation et annulation

- **R-IMP-15** — La validation est **atomique** : soit toutes les décisions du lot sont appliquées,
  soit aucune. Les transactions créées ont l'origine `IMPORT`, le statut `CLEARED` et une
  ventilation conforme à la suggestion (ou non catégorisée).
- **R-IMP-16** — Si le lot porte des soldes de relevé, la validation crée ou met à jour les points de
  contrôle correspondants (spec 04, R-RAP-2). Un point de contrôle à écart nul est proposé au
  rapprochement en un clic (R-RAP-11).
- **R-IMP-17** — Un lot `COMMITTED` peut être **annulé** (`REVERTED`) tant qu'aucune de ses transactions
  n'est `RECONCILED` : les transactions créées sont supprimées, les transactions pointées repassent à
  leur état précédent (statut, date et libellé bancaire), et les points de contrôle créés par le lot
  sont supprimés.
- **R-IMP-21** — Le **fichier source** de chaque lot est conservé sur le serveur (`/data/imports`),
  référencé par le lot, et supprimable à la main (le lot et ses lignes restent). Il peut être inclus
  dans l'export (ADR-0008).
- **R-IMP-18** — Contrôle interne du relevé : si le relevé fournit un solde de début et un solde de
  fin, la revue affiche `fin − début − Σ lignes valides`. Un écart non nul signale une ligne mal lue
  ou manquante **avant** la validation.

### Règles de catégorisation

```jsonc
// conditions : toutes doivent être vraies (ET)
{
  "label":   { "op": "contains" | "starts_with" | "equals" | "regex", "value": "NETFLIX", "field": "bank_label" | "label" },
  "amount":  { "op": "eq" | "between" | "lt" | "gt", "value": -1399, "max": null },  // en centimes
  "sign":    "debit" | "credit",
  "accounts": ["<account_id>"]                                                        // vide = tous
}
// actions
{
  "set_label": "Netflix",
  "set_splits": [ { "category_id": "<id>", "ratio": 1.0 } ],   // ratio ou montant fixe + reste
  "set_project": "<project_id>",                               // rattache à un projet actif (ADR-0014)
  "mark_transfer_to": "<account_id>",                          // suggère un virement interne
  "set_note": null
}
```

- **R-IMP-19** — Les règles s'appliquent aussi **à la demande** sur des transactions existantes non
  catégorisées, avec un aperçu avant application.
- **R-IMP-20** — Depuis une transaction, « créer une règle à partir de celle-ci » préremplit la
  condition sur le libellé normalisé et l'action sur sa ventilation.

## Features

| ID | Priorité | Feature |
|---|---|---|
| IMP-01 | MVP | Importer un CSV/XLSX avec un assistant de mapping et un aperçu en direct |
| IMP-02 | MVP | Profils d'import enregistrés et réutilisables |
| IMP-03 | MVP | Détection des doublons par empreinte |
| IMP-04 | MVP | Écran de revue : décision par ligne (créer / pointer / ignorer), filtres, actions en masse |
| IMP-05 | MVP | Pointage des transactions `PENDING` existantes (vérification) |
| IMP-06 | MVP | Validation atomique et historique des lots |
| IMP-07 | V1 | Annulation d'un lot validé |
| IMP-08 | V1 | Règles de catégorisation (création, priorité, aperçu, application à la demande) |
| IMP-09 | V1 | Import OFX |
| IMP-10 | V1 | Import PDF LCL (parseur dédié) |
| IMP-11 | V1 | Suggestions de virements internes entre lots / comptes |
| IMP-12 | V2 | Import PDF générique par OCR (opt-in) |
| IMP-13 | V2 | Suggestion de catégorie apprise (au-delà de R-IMP-13) |

### IMP-01 — Assistant de mapping

- Étape 1 : déposer le fichier, choisir le compte, choisir un profil existant ou « nouveau profil ».
- Étape 2 (nouveau profil) : encodage et séparateur auto-détectés, aperçu des 20 premières lignes brutes.
- Étape 3 : associer les colonnes aux champs (date, libellé(s), montant ou débit/crédit), choisir les
  formats. L'aperçu affiche **en direct** les lignes normalisées et les erreurs.
- Étape 4 : enregistrer le profil sous un nom, puis lancer la revue.

Critères d'acceptation :
- Étant donné un CSV en windows-1252, séparé par `;`, avec des colonnes « Débit » et « Crédit »
  où « 1 234,56 » figure dans « Débit », alors le montant normalisé vaut −123456 centimes.
- Étant donné un fichier déjà importé, quand je le réimporte, alors toutes ses lignes sont
  `DUPLICATE` / `SKIP` et la validation ne crée aucune transaction.

### IMP-05 — Vérifier des transactions saisies

- Étant donné une transaction manuelle « Boulangerie » de −4,20 € le 2026-03-10 (`PENDING`), quand
  j'importe un relevé contenant « CB BOULANGERIE DUPONT 10/03 » de −4,20 € le 2026-03-11, alors la
  ligne est proposée en `MATCH` (score ≥ 80). Après validation, la transaction est `CLEARED`, garde
  sa `date` 2026-03-10 et le libellé « Boulangerie », et reçoit `bank_date` 2026-03-11 et le libellé
  bancaire. Un point de contrôle au soir du 2026-03-10 ne l'inclut pas.
