# Héritage de la première version

Statut : **Validée** (2026-10-20)

Ce qui mérite d'être repris de la première version (FastAPI + Streamlit, dépôt
`github.com/quentinpigne/cashflow`), et ce qui ne doit pas l'être. Ce document permet de se
passer de l'ancien dépôt une fois le nouveau initialisé.

## ⚠️ Actions de sécurité sur l'ancien dépôt

À faire indépendamment de la réécriture :
1. `packages/backend/.env`, qui contient `MISTRAL_APIKEY`, et `packages/frontend/.streamlit/secrets.toml`
   sont **versionnés** et le dépôt est poussé sur GitHub. **Révoquer la clé Mistral** et en générer une nouvelle.
2. Les retirer du suivi (`git rm --cached`) et les ajouter au `.gitignore`. Envisager de purger
   l'historique, ou de rendre ou garder le dépôt privé.
3. Ne pas committer la modification en cours de la migration Alembic initiale : elle contient un IBAN réel.

## À reprendre

### Parseur de relevés PDF LCL (→ IMP-10)

Logique éprouvée (`app/utils/pdf_importer/lcl_pdf_importer.py`), à porter :
- L'ancien code passait par l'OCR (tables HTML renvoyées par Mistral). Le nouveau parseur part du
  **texte extrait localement avec sa position** (ADR-0007) et reconstruit les colonnes à partir de
  l'abscisse des en-têtes. L'OCR reste un repli. La logique ci-dessous reste valable dans les deux cas.
- La **table des opérations** est celle dont les en-têtes contiennent `DATE`, `LIBELLE`, `DEBIT` et
  `CREDIT` (l'ancien code essayait avec 1 puis 0 ligne sautée avant l'en-tête).
- Période du relevé lue sur la première page : `du (\d+\.\d+\.\d+) au \d+\.\d+\.\d+` (format `dd.MM.yyyy`).
- Lignes à exclure : `ANCIEN SOLDE`, `TOTAUX`, `SOLDE EN EUROS`, `SOLDE INTERMEDIAIRE A …`.
  ⚠️ Les lignes `ANCIEN SOLDE` et `SOLDE EN EUROS` portent justement les **soldes de début et de fin** :
  le nouveau parseur doit les **lire** pour alimenter R-IMP-18 et R-RAP-2, pas seulement les filtrer.
- Une ligne **sans date** est la **suite du libellé** de la ligne précédente (concaténer avec un saut de ligne).
- Date : si le libellé contient `CB … dd/MM/yy`, prendre cette date (date réelle de l'achat carte) ;
  sinon, `dd.MM` de la colonne DATE, avec l'année de la période, en passant à l'année suivante pour
  janvier quand le relevé commence en décembre (R-IMP-4).
- Montant : `CREDIT` s'il est renseigné, sinon `−DEBIT` ; séparateur de milliers espace, décimale virgule.
  ⚠️ L'ancien code passait par `float` : le nouveau doit convertir depuis la chaîne (R-IMP-2).

Point à valider : la date « CB » est la date d'achat, et non la date d'opération du relevé. Il faut
choisir laquelle va dans `date` et laquelle dans `value_date`. Proposition : date d'achat dans `date`
(plus parlante pour le budget), date du relevé dans `value_date`.

### Extraction générique par annotation (→ IMP-12)

Schéma demandé au modèle OCR (`BankStatement`) : une liste de transactions `{date: "dd/mm/yyyy",
label, amount}`, avec la consigne qu'une ligne sans date est la continuation du libellé précédent et
que le montant est négatif pour un débit. Il faut l'enrichir avec les soldes de début et de fin et la période.

### Référentiel des établissements (→ données initiales)

| Établissement | Code banque |
|---|---|
| BNP Paribas | 30004 |
| Société Générale | 30003 |
| Crédit Agricole | 30006 |
| Crédit Mutuel | — |
| CIC | — |
| CIC - Épargne Salariale | — |
| La Banque Postale | 20041 |
| Crédit du Nord | 30076 |
| LCL | 30002 |
| HSBC France | — |
| Boursorama Banque | 40618 |
| Fortuneo | — |
| ING | — |
| Monabanq | 14690 |
| Hello bank! | — |
| BforBank | 16218 |
| Orange Bank | 18370 |
| N26 | 20433 |
| Revolut | 28233 |

À vérifier avant de les reprendre : certains établissements ont fusionné ou cessé leur activité
(Crédit du Nord a été absorbé par la Société Générale, Orange Bank a été repris). Les codes sont à
contrôler à la source officielle.

### Catégories par défaut (→ R-TRX-23)

Dépenses :
- **Logement** : Loyer · Gaz, électricité, eau · Décoration · Extérieur et jardin · Autres
- **Alimentation & restauration** : Supermarché / épicerie · Restaurants · Cafés / bars · Fast food · Autres
- **Achats & shopping** : Vêtements / chaussures · Autres
- **Auto & transports** : Carburant · Péage · Entretien · Autres
- **Abonnements** : Internet · Téléphone · Autres

Revenus : Salaires · Allocations et pensions · Autres revenus

Remarques :
- Dans l'ancien seed, les sous-catégories d'« Abonnements » pointaient vers le mauvais parent
  (`parent_id: 22` au lieu de 21). Le nouveau seed doit référencer les parents par nom ou par
  identifiant stable, jamais par entier positionnel.
- La catégorie de revenu « Remboursements » de l'ancien seed n'est pas reprise : un remboursement vient
  en déduction de la catégorie de dépense d'origine (R-TRX-13).
- À compléter : Santé, Loisirs, Voyages, Impôts et taxes, Assurances, Enfants, Animaux, Dons, Frais
  bancaires, Prêts (dépense, utilisée par R-CPT-12), Intérêts (revenu, utilisé par R-EPA-10).

### Maquette du tableau de bord

La disposition de l'ancien écran d'accueil (indicateurs en haut ; comptes à gauche ; budget,
projection et performance au centre ; indicateurs clés à droite) est une bonne base pour TDB-01.
Tous ses chiffres étaient en dur.

## À ne pas reprendre

| Élément | Raison | Remplacé par |
|---|---|---|
| Montants en `float` | Inexactitude | ADR-0001 |
| `current_balance` stocké et modifié | Dérive silencieuse | ADR-0002 |
| `category_id` sur la transaction | Pas de ventilation | ADR-0003 |
| Import qui écrit directement | Pas de revue, doublons | ADR-0005 |
| REST et GraphQL en parallèle | Double maintenance | Une seule interface (ADR-0009) |
| Coordonnées bancaires obligatoires sur `Account` | Inadaptées à l'épargne et aux espèces | Champs optionnels (spec 01) |
| `datetime` pour les dates d'opération | Glissements de fuseau | ADR-0012 |
| Données personnelles dans les migrations | Fuite | Seed séparé, sans données personnelles (architecture) |
| Score financier, allocation actions/obligations/crypto | Arbitraire ou hors périmètre V1 | — (vision, ADR-0010) |
