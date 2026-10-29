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
| PDF (parseur dédié à une banque) | V1 | Parseur spécifique (LCL d'abord, R-IMP-23 à R-IMP-26) sur le texte extrait **localement** avec sa position. Pas de mapping ni de profil. OCR (opt-in) en repli pour les PDF scannés (ADR-0007, V2) |
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
  libellé définitif), ou si la même période est importée sous deux formats (CSV puis PDF), R-IMP-27
  signale un doublon probable et la décision revient à l'utilisateur.

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
  `bank_date` = date de la ligne et `date` = **date d'achat** de la ligne si le relevé la donne (paiement par
  carte d'un relevé LCL, R-IMP-26), sinon la date de la ligne.

- **R-IMP-27** — **Doublon probable.** Une ligne ni `DUPLICATE` (R-IMP-6) ni associée à un candidat (R-IMP-9)
  est comparée aux transactions **déjà importées** du compte (avec une empreinte) dont la date de solde tombe
  **dans la période du relevé** : même montant exact, date à ±5 jours (le même réglage que R-IMP-9), score de
  R-IMP-9, chaque transaction associée à une seule ligne. La ligne est marquée `DUPLICATE`, avec la transaction
  trouvée et son score, et sa décision par défaut est `SKIP` ; l'utilisateur peut la créer quand même. Se
  limiter à la période du relevé évite de confondre une opération du nouveau mois avec une opération semblable
  du mois précédent.

### Suggestions

- **R-IMP-12** — Les règles de catégorisation actives sont évaluées par `priority` croissante sur chaque
  ligne `CREATE`. La première règle qui correspond s'applique ; si son `stop_processing` vaut `false`,
  les suivantes sont aussi évaluées, et les actions déjà appliquées ne sont pas écrasées.
- **R-IMP-13** — Sans règle applicable, la catégorie suggérée est celle de la dernière transaction du
  même compte de même **clé d'apprentissage**, si elle existe (« apprentissage » minimal). Le tiers suit
  une règle voisine, tous comptes confondus (R-TIE-17, R-TIE-18).
  - La clé d'apprentissage est le libellé bancaire normalisé (R-IMP-5), découpé en mots (tout caractère
    non alphanumérique sépare), sans les mots qui contiennent un chiffre (dates, numéros de carte,
    montants, références) : « CB CARREFOUR MEYLAN 14/03 » et « CB Carrefour Meylan 02/04 » ont la même
    clé, « CB CARREFOUR MEYLAN ».
  - S'il ne reste que des mots d'opération (CB, CARTE, PRLV, PRELEVEMENT, SEPA, VIR, VIREMENT, RECU,
    EMIS, INST, PAIEMENT, RETRAIT, DAB, ECH, FACT, FACTURE, EUR), la ligne n'a pas de clé et rien n'est
    appris.
- **R-IMP-14** — Les candidats de virement interne (R-TRX-35), y compris parmi les lignes d'**autres
  lots en revue**, sont suggérés. Accepter la suggestion crée le virement à la validation. Sans
  candidat, marquer la ligne comme virement vers un compte crée la jambe opposée à la validation
  (R-TRX-36).
- **R-IMP-22** — Après la validation, les transactions du lot (créées ou pointées) qui correspondent à une
  échéance de prêt pas encore payée (R-PRE-6) sont proposées sur l'écran de fin d'import, comme le
  rapprochement (R-RAP-11). Accepter les ventile en remboursement, intérêts et assurance. Rien n'est ventilé
  sans cet accord.

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
- **R-IMP-28** — À la validation d'un lot dont le fichier source est un **PDF**, ce fichier est aussi archivé
  dans la bibliothèque comme **relevé du compte** (R-CPT-18, R-DOC-17), avec la période du lot, au mois qui couvre le plus de jours de cette
  période (un relevé LCL du 3 septembre au 2 octobre est celui de septembre ; à égalité, le plus ancien) ; sans
  période, au mois de la date de fin du lot, à défaut au mois de la validation. Son titre est « Relevé <compte>
  <mois> » (« Relevé Compte courant septembre 2026 »). Le relevé est une copie : il a sa propre vie, indépendante du lot et de
  son fichier source (R-CPT-20). Un fichier déjà dans la bibliothèque ne l'est pas une seconde fois
  (R-CPT-19, R-DOC-3).
- **R-IMP-18** — Contrôle interne du relevé : si le relevé fournit un solde de début et un solde de
  fin, la revue affiche `fin − début − Σ lignes valides`. Un écart non nul signale une ligne mal lue
  ou manquante **avant** la validation.

### Relevé PDF LCL (IMP-10)

- **R-IMP-23** — Le texte du PDF et la position de chaque élément sont extraits **localement**, sans service tiers
  (ADR-0007). Un PDF sans texte extractible (scanné) ou qui n'est pas un relevé de compte LCL reconnu est refusé
  avec un message explicite ; aucun lot n'est créé. Le relevé n'a pas de profil d'import.
- **R-IMP-24** — **Table des opérations.** Sur chaque page, la table commence sous la ligne d'en-têtes `DATE`,
  `LIBELLE`, `VALEUR`, `DEBIT`, `CREDIT` ; l'abscisse des en-têtes donne les colonnes. Un montant au crédit est
  positif, au débit négatif (R-IMP-2). Une ligne **sans date** est la suite du libellé de la ligne précédente
  (ajoutée après un saut de ligne) ; le libellé de la transaction créée n'en reprend que la première ligne, le
  libellé bancaire garde tout. Le pied de page (« Page n / m ») est ignoré.
- **R-IMP-25** — **Soldes et période.** La période est lue en tête (`du jj.mm.aaaa au jj.mm.aaaa`). La ligne
  `ANCIEN SOLDE` donne le solde de début, `SOLDE EN EUROS` le solde de fin et sa date ; elles ne sont pas des
  opérations. `SOLDE INTERMEDIAIRE …` et `TOTAUX` sont ignorées. La lecture s'arrête après `SOLDE EN EUROS`.
- **R-IMP-26** — **Dates.** La colonne `DATE` (`jj.mm`, année selon R-IMP-4) donne la date de la ligne, qui
  devient la date bancaire ; la colonne `VALEUR` (`jj.mm.aa`) la date de valeur. Pour un paiement par carte
  (libellé « CB … jj/mm/aa »), la date du libellé est la **date d'achat**, qui devient la date de la
  transaction créée (R-IMP-11). Les soldes et le rapprochement restent ainsi calés sur la banque (R-TRX-7).

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
  "set_payee": "<payee_id>",                                   // tiers existant de la transaction (spec 11, R-TIE-16)
  "mark_transfer_to": "<account_id>",                          // suggère un virement interne
  "set_note": null
}
```

- **R-IMP-19** — Les règles s'appliquent aussi **à la demande** sur des transactions existantes non
  catégorisées (ou sans tiers, pour une règle qui en affecte un, R-TIE-19), avec un aperçu avant application.
- **R-IMP-20** — Depuis une transaction, « créer une règle à partir de celle-ci » préremplit la
  condition sur le libellé normalisé et les actions sur sa ventilation et son tiers.

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
