# Spec 11 — Tiers

Statut : **Validée** (2026-10-23 ; adresse et lieu : 2026-10-28)
Modèle : [Payee, PayeeGroup](../modele-de-donnees.md#tiers--spec-11)
Décision : [ADR-0025](../adr/0025-adresse-des-tiers-et-geocodage.md) (adresse et géocodage)

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
  les actions de règle (`set_payee`) de A, ainsi que ses rattachements de documents (R-DOC-13), puis
  supprime A. La confirmation indique le nombre de transactions concernées. La fusion est atomique.

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
  tiers, le tiers suggéré est celui de la dernière transaction, **tous comptes confondus**, de même clé
  d'apprentissage que le libellé bancaire (R-IMP-13), s'il existe et n'est pas archivé.
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
  sont exclues, comme partout (R-TRX-37), ainsi que celles d'une catégorie hors analyses (R-TRX-24). Rien
  n'est stocké.
- **R-TIE-21** — Le non catégorisé d'un tiers est affiché à côté de ses dépenses et revenus, jamais ajouté
  à l'un ou l'autre : un total ne cache pas une transaction à catégoriser.
- **R-TIE-22** — Comme les autres agrégats, ceux des tiers portent sur les comptes en EUR (R-CPT-2), et les
  flux sont rattachés à la période par leur `date` (R-TRX-7).

## Adresse et lieu

Le **lieu** d'une transaction est l'adresse de son tiers : c'est le magasin qui est à Meylan, pas le
paiement. Décision : ADR-0025.

- **R-TIE-23** — Un tiers a optionnellement une **adresse** : ligne d'adresse (≤ 200 caractères), code postal
  (≤ 10), ville (≤ 80) et pays (code ISO 3166-1 alpha-2, « FR » proposé à la saisie). Dès qu'un champ de
  l'adresse est renseigné, la **ville** et le **pays** sont requis. Un code postal français a 5 chiffres.
- **R-TIE-24** — Le **département** d'une adresse française se déduit de son code postal, rien n'est stocké :
  les deux premiers chiffres (« 38240 » → 38), sauf la Corse (200xx et 201xx → 2A, 202xx à 206xx → 2B) et
  l'outre-mer (971 à 974 et 976, sur trois chiffres). Un autre code (98xxx…) n'a pas de département.
- **R-TIE-25** — Une adresse peut porter des **coordonnées** (latitude et longitude en millionièmes de degré,
  des entiers). Il n'y a pas de coordonnées sans adresse. Elles viennent de la recherche d'adresse
  (R-TIE-26) : modifier ensuite un champ de l'adresse à la main les efface, elles ne lui correspondraient
  plus.
- **R-TIE-26** — **Recherche d'adresse** (géocodage) : désactivée par défaut, elle s'active dans les réglages.
  Le texte saisi est envoyé par le serveur à la Base Adresse Nationale, ce que le champ indique (ADR-0007).
  Choisir une proposition remplit la ligne d'adresse, le code postal, la ville, le pays (FR) et les
  coordonnées. Elle ne couvre que la France ; désactivée ou injoignable, l'adresse se saisit à la main.
- **R-TIE-27** — Fusionner un tiers dans un autre (R-TIE-7) garde l'adresse du tiers cible ; s'il n'en a pas,
  il reprend celle du tiers fusionné.
- **R-TIE-28** — **Analyse par lieu** : sur une période, les dépenses (ou les revenus) et le non catégorisé
  des tiers (R-TIE-20, R-TIE-21) sont regroupés par **pays**, par **département** ou par **ville** de leur
  adresse. Une ville est identifiée par son pays, son département et son nom sans distinction de casse. Les
  tiers sans adresse forment la ligne « Sans adresse » ; par département, les adresses à l'étranger forment
  la ligne « Étranger » et celles dont le département ne se déduit pas, « Département inconnu ». Chaque lieu
  se déplie en ses tiers. Comme l'analyse par tiers, elle ne porte que sur les transactions qui ont un tiers.
- **R-TIE-29** — **Carte** : les départements de France métropolitaine sont coloriés selon le montant de la
  période (R-TIE-28, par département), et les tiers qui ont des coordonnées y sont placés, d'une taille
  proportionnelle à leur montant. Le fond de carte est embarqué dans l'application : aucun service de
  cartographie n'est appelé. Le nombre de tiers de la période absents de la carte (sans coordonnées, outre-mer,
  étranger) est indiqué.

## Features

| ID | Priorité | Feature |
|---|---|---|
| TIE-01 | V1 | Gérer les tiers et les groupes : création, renommage, groupe, archivage, fusion, suppression |
| TIE-02 | V1 | Affecter un tiers : saisie (avec création à la volée), détail, récurrences, en masse ; suggestion de catégorie |
| TIE-03 | V1 | Tiers à l'import : action de règle `set_payee`, suggestion apprise, revue d'import |
| TIE-04 | V1 | Dépenses et revenus par tiers et par groupe : analyse, fiche d'un tiers ou d'un groupe, filtre des transactions |
| TIE-05 | V2 | Proposer des tiers à partir des libellés bancaires (libellés proches regroupés) |
| TIE-06 | V1 | Adresse d'un tiers : saisie, recherche d'adresse par la BAN (activable), affichage sur la fiche |
| TIE-07 | V1 | Dépenses et revenus par lieu : par pays, département ou ville, et carte |

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
- Étant donné une transaction « CB BOULANGERIE DUPONT 02/03 » rattachée au tiers « Boulangerie Dupont »,
  quand j'importe une ligne « CB BOULANGERIE DUPONT 10/03 », alors le tiers suggéré est « Boulangerie
  Dupont » : la date du libellé ne compte pas (R-TIE-17, R-IMP-13). Une ligne « CB 11/03 4521 » ne
  reçoit aucune suggestion.

### TIE-04 — Critères d'acceptation

- Étant donné en mars −80 € chez « Carrefour Meylan » (Alimentation), −20 € chez « Carrefour Market
  Grenoble » (Alimentation), +5 € remboursés chez « Carrefour Meylan » (Alimentation) et −15 € non
  catégorisés chez « Carrefour Meylan », tous deux tiers du groupe « Carrefour », alors en mars : dépenses du
  groupe = 95 €, non catégorisé = 15 € en sorties, et dépenses de « Carrefour Meylan » = 75 € (R-TIE-20,
  R-TIE-21).

### TIE-06 — Critères d'acceptation

- Étant donné un tiers, quand je saisis le code postal « 38240 » sans ville, alors l'enregistrement est refusé
  (R-TIE-23).
- Étant donné la recherche d'adresse activée, quand je choisis la proposition « 8 Boulevard du Port 80000
  Amiens », alors le tiers a pour ville « Amiens », pour pays FR et des coordonnées (R-TIE-26).
- Étant donné un tiers géocodé, quand je corrige son code postal à la main, alors ses coordonnées sont
  effacées (R-TIE-25).
- Étant donné la recherche d'adresse désactivée, quand j'appelle la recherche, alors elle est refusée et
  aucune requête ne part vers la BAN (R-TIE-26).

### TIE-07 — Critères d'acceptation

- Étant donné en mars −40 € chez « Boulangerie » (Meylan, 38240), −60 € chez « Carrefour Meylan » (MEYLAN,
  38240), −30 € chez « Fnac » (Grenoble, 38000) et −10 € chez « Amazon » (sans adresse), alors par ville :
  Meylan = 100 € (2 tiers), Grenoble = 30 €, Sans adresse = 10 € ; par département : Isère (38) = 130 €
  (R-TIE-24, R-TIE-28).
- Étant donné une adresse en Corse « 20090 Ajaccio », alors son département est 2A (R-TIE-24).
