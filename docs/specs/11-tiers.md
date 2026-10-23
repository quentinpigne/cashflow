# Spec 11 — Tiers

Statut : **Validée** (2026-10-23)
Modèle : [Payee, PayeeGroup](../modele-de-donnees.md#tiers--spec-11)

## Concepts

Un **tiers** (`Payee`) est la personne ou l'organisme de l'autre côté d'une transaction : le magasin où
j'ai payé, l'employeur qui me verse mon salaire, l'administration, un proche. Il répond à la question
« **avec qui** ? », là où la catégorie répond à « **pour quoi** ? ». Les deux sont indépendants : chez
Carrefour, j'achète de l'alimentation, de l'entretien ou un cadeau.

Un tiers peut appartenir à un **groupe de tiers** (`PayeeGroup`) : « Carrefour Meylan » et « Carrefour
Market Grenoble » appartiennent au groupe « Carrefour ». Le groupe permet de savoir ce que je dépense dans
une enseigne, quel que soit le magasin. Un groupe peut aussi réunir des personnes ou des organismes
(« Famille », « Médecins »).

```
Groupe de tiers  Carrefour
 ├─ Tiers  Carrefour Meylan          ← transactions
 └─ Tiers  Carrefour Market Grenoble ← transactions
Tiers  Boulangerie Dupont            (sans groupe)
```

Le tiers est **facultatif** et porté par la **transaction**, pas par la ventilation : un ticket ventilé
entre plusieurs catégories reste un seul passage en caisse.

Le libellé bancaire (« CB CARREFOUR MEYLAN 12/03 ») n'est pas un tiers : il est bruité et varie d'une
opération à l'autre. Les règles de catégorisation font le lien entre les libellés et les tiers.

## Tiers et groupes

- **R-TIE-1** — Un **tiers** a un nom (≤ 80 caractères), unique sans distinction de casse. Il a
  optionnellement un groupe et une note.
- **R-TIE-2** — Un **groupe de tiers** a un nom (≤ 80 caractères), unique sans distinction de casse, et
  optionnellement une note. Il n'y a qu'**un niveau** : un groupe ne contient que des tiers, pas d'autres
  groupes. Un tiers appartient au plus à un groupe.
- **R-TIE-3** — On peut changer le groupe d'un tiers à tout moment. Les totaux du groupe se recalculent :
  rien n'est stocké (R-TIE-20).
- **R-TIE-4** — **Supprimer un groupe** est toujours possible, après confirmation qui indique le nombre de
  tiers concernés : ses tiers restent, sans groupe.
- **R-TIE-5** — Un tiers **utilisé** (par une transaction, une récurrence ou une règle) ne se supprime pas :
  il s'**archive** ou se **fusionne** dans un autre (même logique que R-TRX-22). Un tiers jamais utilisé se
  supprime.
- **R-TIE-6** — Un tiers **archivé** n'est plus proposé en saisie. Ses transactions, ses récurrences et les
  règles qui l'affectent restent inchangées. Il reste visible dans les analyses et les filtres.
- **R-TIE-7** — **Fusionner** le tiers A dans le tiers B réaffecte à B les transactions, les récurrences et
  les actions de règle (`set_payee`) de A, puis supprime A. La confirmation indique le nombre de transactions
  concernées. La fusion est atomique.

## Tiers d'une transaction

- **R-TIE-10** — Une transaction a **au plus un** tiers, jamais un groupe. Une transaction sans tiers est
  normale : rien ne le signale comme un manque, contrairement à une ventilation non catégorisée.
- **R-TIE-11** — Une **jambe de virement interne** n'a pas de tiers : de l'autre côté, c'est un de mes
  comptes (ADR-0004). Lier deux transactions en virement retire leur tiers, après la même confirmation
  que pour leurs catégories (R-TRX-33). Une transaction qui porte une ventilation de remboursement ou
  d'acquisition peut avoir un tiers (la banque prêteuse, le vendeur).
- **R-TIE-12** — Le tiers d'une transaction `RECONCILED` reste modifiable : il n'affecte pas le solde
  (R-TRX-3).
- **R-TIE-13** — Une **récurrence** peut porter un tiers (le propriétaire pour un loyer). Une récurrence de
  virement (`target_account_id`) n'en a pas (R-TIE-11). Matérialiser une échéance (R-TRX-43) reprend le tiers
  de la récurrence.
- **R-TIE-14** — **Suggestion de catégorie** : à la saisie, choisir un tiers alors que la catégorie est vide
  préremplit la catégorie de la dernière transaction de ce tiers (par `date`) qui n'a qu'une ventilation,
  catégorisée. Rien n'est stocké sur le tiers ; une catégorie déjà choisie n'est jamais remplacée.
- **R-TIE-15** — Le tiers se choisit **ou se crée** depuis la saisie : taper un nom inconnu propose
  « Créer le tiers « … » », sans groupe.

## Import et règles

- **R-TIE-16** — Les règles de catégorisation ont une action **`set_payee`** (spec 03). Elle suit R-IMP-12 :
  une règle qui s'applique après une autre n'écrase pas le tiers déjà proposé. Une règle ne crée jamais de
  tiers : elle désigne un tiers existant.
- **R-TIE-17** — **Suggestion apprise** : pour une ligne `CREATE` à laquelle aucune règle n'affecte de
  tiers, le tiers suggéré est celui de la dernière transaction, **tous comptes confondus**, au libellé
  bancaire normalisé identique (même normalisation que R-IMP-13), s'il existe et n'est pas archivé.
- **R-TIE-18** — Si aucune règle ni R-IMP-13 ne suggère de catégorie, mais qu'un tiers est suggéré, la
  catégorie suggérée est celle de R-TIE-14 pour ce tiers.
- **R-TIE-19** — L'application **à la demande** d'une règle (R-IMP-19) qui porte `set_payee` vise aussi les
  transactions **sans tiers**, même catégorisées ; elle ne remplace jamais un tiers existant. L'aperçu
  indique, par transaction, ce qui changera (catégorie, tiers ou les deux).

Aucun tiers n'est créé automatiquement à partir des libellés (cf. TIE-05, V2).

## Analyses

- **R-TIE-20** — Pour un tiers, sur une période, les **dépenses**, les **revenus** et le **non catégorisé**
  sont ceux de spec 08 (R-TDB-3, R-TDB-4, R-TDB-5), restreints aux ventilations des transactions de ce tiers.
  Pour un groupe, on somme ceux de ses tiers. Les ventilations de virement, de remboursement et d'acquisition
  sont exclues, comme partout (R-TRX-37). Rien n'est stocké.
- **R-TIE-21** — Le non catégorisé d'un tiers est affiché à côté de ses dépenses et revenus, jamais ajouté
  à l'un ou l'autre : un total ne cache pas une transaction à catégoriser.
- **R-TIE-22** — Comme les autres agrégats, ceux des tiers portent sur les comptes en EUR (R-CPT-2), et les
  flux sont rattachés à la période par leur `date` (R-TRX-7).

## Features

| ID | Priorité | Feature |
|---|---|---|
| TIE-01 | V1 | Gérer les tiers et les groupes : création, renommage, groupe, archivage, fusion, suppression |
| TIE-02 | V1 | Affecter un tiers : saisie (avec création à la volée), détail, récurrences, en masse ; suggestion de catégorie |
| TIE-03 | V1 | Tiers à l'import : action de règle `set_payee`, suggestion apprise, revue d'import |
| TIE-04 | V1 | Dépenses et revenus par tiers et par groupe : analyse, fiche d'un tiers ou d'un groupe, filtre des transactions |
| TIE-05 | V2 | Proposer des tiers à partir des libellés bancaires (libellés proches regroupés) |

### TIE-01 — Critères d'acceptation

- Étant donné le tiers « Carrefour Meylan », quand je crée « CARREFOUR MEYLAN », alors la création est
  refusée (R-TIE-1).
- Étant donné « Carrefour Meylan » utilisé par 12 transactions et une règle, quand je le fusionne dans
  « Carrefour Market Meylan », alors les 12 transactions et la règle désignent « Carrefour Market Meylan », et
  « Carrefour Meylan » n'existe plus (R-TIE-7).
- Étant donné le groupe « Carrefour » de 3 tiers, quand je le supprime, alors les 3 tiers existent toujours,
  sans groupe (R-TIE-4).

### TIE-02 — Critères d'acceptation

- Étant donné une dernière transaction « Carrefour Meylan » catégorisée « Alimentation › Supermarché »,
  quand je saisis une dépense et choisis ce tiers sans catégorie, alors la catégorie proposée est
  « Alimentation › Supermarché » (R-TIE-14).
- Étant donné une transaction avec un tiers, quand je la lie en virement à une autre, alors elle n'a plus de
  tiers (R-TIE-11).
- Étant donné une transaction rapprochée, quand je change son tiers, alors la modification est acceptée sans
  déverrouillage (R-TIE-12).

### TIE-03 — Critères d'acceptation

- Étant donné la règle « libellé contient CARREFOUR MEYLAN → tiers Carrefour Meylan », quand j'importe une
  ligne « CB CARREFOUR MEYLAN 14/03 », alors le tiers suggéré est « Carrefour Meylan » (R-TIE-16).
- Étant donné une transaction du livret au libellé normalisé « PRLV EDF » rattachée au tiers « EDF », quand
  j'importe une ligne « PRLV EDF » sur le compte courant et qu'aucune règle ne s'applique, alors le tiers
  suggéré est « EDF » (R-TIE-17).

### TIE-04 — Critères d'acceptation

- Étant donné en mars −80 € chez « Carrefour Meylan » (Alimentation), −20 € chez « Carrefour Market
  Grenoble » (Alimentation), +5 € remboursés chez « Carrefour Meylan » (Alimentation) et −15 € non
  catégorisés chez « Carrefour Meylan », tous deux tiers du groupe « Carrefour », alors en mars : dépenses du
  groupe = 95 €, non catégorisé = 15 € en sorties, et dépenses de « Carrefour Meylan » = 75 € (R-TIE-20,
  R-TIE-21).
