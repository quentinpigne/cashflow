# Spec 11 — Tiers

Statut : **Validée** (2026-10-23 ; adresse et lieu : 2026-10-28 ; propositions de tiers : 2026-10-31)
Modèle : [Payee, PayeeGroup, IgnoredPayeeKey](../modele-de-donnees.md#tiers--spec-11)
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

Aucun tiers n'est créé automatiquement à partir des libellés : les propositions de tiers (R-TIE-30 à R-TIE-35)
attendent une validation explicite.

## Analyses

- **R-TIE-20** — Pour un tiers, sur une période, les **dépenses**, les **revenus** et le **non catégorisé**
  sont ceux de spec 08 (R-TDB-3, R-TDB-4, R-TDB-5, rangés par date d'analyse, R-TRX-57), restreints aux
  ventilations des transactions de ce tiers.
  Pour un groupe, on somme ceux de ses tiers. Les ventilations de virement, de remboursement et d'acquisition
  sont exclues, comme partout (R-TRX-37), ainsi que celles d'une catégorie hors analyses (R-TRX-24). Rien
  n'est stocké.
- **R-TIE-21** — Le non catégorisé d'un tiers est affiché à côté de ses dépenses et revenus, jamais ajouté
  à l'un ou l'autre : un total ne cache pas une transaction à catégoriser.
- **R-TIE-22** — Comme les autres agrégats, ceux des tiers portent sur les comptes en EUR (R-CPT-2), et les
  flux sont rattachés à la période par leur `date` (R-TRX-7).

## Adresse et lieu

Le **lieu** d'une transaction est l'adresse de son tiers : c'est le magasin qui est à Meylan, pas le
paiement. Une transaction peut aussi porter sa propre adresse, qui l'emporte sur celle du tiers (spec 02,
R-TRX-59, R-TRX-60). Décisions : ADR-0025, ADR-0029, ADR-0030.

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
  Elle cherche dans le **pays choisi** : le texte saisi est envoyé par le serveur à la Base Adresse Nationale
  pour la France, à Photon (données OpenStreetMap) pour un autre pays, ce que le champ indique (ADR-0007,
  ADR-0030). Choisir une proposition remplit la ligne d'adresse, le code postal, la ville, le pays et les
  coordonnées ; une proposition sans ville n'est pas proposée. Désactivée ou injoignable, l'adresse se saisit
  à la main.
- **R-TIE-27** — Fusionner un tiers dans un autre (R-TIE-7) garde l'adresse du tiers cible ; s'il n'en a pas,
  il reprend celle du tiers fusionné.
- **R-TIE-28** — **Analyse par lieu** : sur une période, les dépenses (ou les revenus) et le non catégorisé
  (R-TIE-20, R-TIE-21) sont regroupés par **pays**, par **département** ou par **ville** du lieu de chaque
  transaction : son adresse, sinon celle de son tiers (R-TRX-60). Une ville est identifiée par son pays, son département et son nom sans distinction de casse. Les
  tiers sans adresse forment la ligne « Sans adresse » ; par département, les adresses à l'étranger forment
  la ligne « Étranger » et celles dont le département ne se déduit pas, « Département inconnu ». Chaque lieu
  se déplie en ses tiers, et en « Sans tiers » pour les transactions qui n'ont que leur adresse. Elle porte sur
  les transactions qui ont un tiers ou une adresse.
- **R-TIE-29** — **Carte** : les départements de France métropolitaine sont coloriés selon le montant de la
  période (R-TIE-28, par département), et les tiers qui ont des coordonnées y sont placés, d'une taille
  proportionnelle à leur montant, ainsi que les adresses de transaction qui en ont (R-TRX-60 ; une adresse
  partagée par plusieurs transactions est un seul point). Le fond de carte est embarqué dans l'application :
  aucun service de cartographie n'est appelé. Le nombre de lieux de la période (tiers ou adresses de
  transaction) absents de la carte (sans coordonnées, outre-mer, étranger) est indiqué.

## Propositions de tiers

Les transactions importées avant de créer ses tiers, ou sans règle, n'ont pas de tiers. Plutôt que de les
affecter une à une, l'application regroupe leurs libellés bancaires et propose un tiers par regroupement.
Rien ne change avant validation.

- **R-TIE-30** — La **clé de tiers** d'un libellé bancaire est sa clé d'apprentissage (R-IMP-13) sans les mots
  qui décrivent le type d'opération (`CB`, `CARTE`, `PRLV`, `SEPA`, `VIR`, `RETRAIT`…, la liste de R-IMP-13) :
  « CB CARREFOUR MEYLAN 14/03 » et « PRLV SEPA CARREFOUR MEYLAN » ont la clé « CARREFOUR MEYLAN ». Un libellé
  qui n'a pas de clé d'apprentissage n'a pas de clé de tiers. Deux libellés sont **proches** s'ils ont la même
  clé de tiers : il n'y a pas de rapprochement approché.
- **R-TIE-31** — Une **proposition de tiers** regroupe les transactions **sans tiers**, hors jambes de virement
  (R-TIE-11), tous comptes confondus, qui ont la même clé de tiers, à condition qu'elles soient **au moins
  deux** et que la clé ne soit pas ignorée (R-TIE-35). Elle indique le nombre de transactions, la date de la
  première et de la dernière, et jusqu'à trois libellés bancaires distincts, les plus récents. Les propositions
  sont rangées par nombre de transactions décroissant, puis par clé. Rien n'est stocké : elles se recalculent
  à chaque consultation, et une proposition dont il ne reste plus qu'une transaction sans tiers disparaît.
- **R-TIE-32** — Une proposition suggère un **nom** : sa clé, chaque mot avec une majuscule initiale
  (« CARREFOUR MEYLAN » → « Carrefour Meylan »), tronquée à 80 caractères. Elle suggère un **tiers existant**
  à la place, s'il y en a un : celui, non archivé, de la dernière transaction (par `date`) de même clé de tiers
  qui en a un ; à défaut, le tiers non archivé qui porte ce nom, sans distinction de casse.
- **R-TIE-33** — **Valider** une proposition affecte un tiers à toutes ses transactions sans tiers, y compris
  rapprochées (R-TIE-12) : un tiers existant non archivé choisi, ou un tiers créé sous le nom saisi, sans
  groupe (R-TIE-1 : un nom déjà pris est refusé). Les catégories ne changent pas. La validation est atomique et
  indique le nombre de transactions modifiées.
- **R-TIE-34** — À la validation, l'option **« Créer aussi une règle »**, cochée par défaut, crée une règle de
  catégorisation pour les imports suivants : condition « libellé bancaire correspond à `*MOT1*MOT2*…*` »
  (opérateur `regex` de spec 03, où `*` remplace toute suite de caractères :
  les mots de la clé, dans l'ordre), action `set_payee` seule, sans arrêt du traitement, priorité
  avant les règles existantes : elle n'affecte que le tiers et laisse les suivantes compléter la catégorie
  (R-IMP-12), même celles qui arrêtent le traitement.
  Elle s'appelle « Tiers : <nom du tiers> ». Sans règle, la suggestion apprise (R-TIE-17) prend déjà le
  relais sur les libellés de même clé d'apprentissage.
- **R-TIE-35** — **Ignorer** une proposition mémorise sa clé : elle n'est plus proposée (« FRAIS TENUE COMPTE »,
  « COTISATION »), quelles que soient les transactions qui arrivent ensuite. Les clés ignorées se consultent et se
  **rétablissent** ; rétablir une clé la propose de nouveau si elle remplit R-TIE-31.

## Features

| ID | Priorité | Feature |
|---|---|---|
| TIE-01 | V1 | Gérer les tiers et les groupes : création, renommage, groupe, archivage, fusion, suppression |
| TIE-02 | V1 | Affecter un tiers : saisie (avec création à la volée), détail, récurrences, en masse ; suggestion de catégorie |
| TIE-03 | V1 | Tiers à l'import : action de règle `set_payee`, suggestion apprise, revue d'import |
| TIE-04 | V1 | Dépenses et revenus par tiers et par groupe : analyse, fiche d'un tiers ou d'un groupe, filtre des transactions |
| TIE-05 | V2 | Proposer des tiers à partir des libellés bancaires (libellés proches regroupés), avec règle optionnelle |
| TIE-06 | V1 | Adresse d'un tiers : saisie, recherche d'adresse (activable ; BAN en France, Photon à l'étranger), affichage sur la fiche |
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

### TIE-05 — Critères d'acceptation

- Étant donné trois transactions sans tiers « CB CARREFOUR MEYLAN 14/03 », « CB CARREFOUR MEYLAN 02/04 »
  (compte courant) et « PRLV SEPA CARREFOUR MEYLAN » (compte joint), alors une proposition « Carrefour
  Meylan » regroupe ces trois transactions (R-TIE-30, R-TIE-31).
- Étant donné une seule transaction sans tiers « CB BOULANGERIE DUPONT 02/03 », alors aucune proposition ne la
  concerne (R-TIE-31).
- Étant donné une transaction « CB EDF 05/01 » rattachée au tiers « EDF » et deux transactions sans tiers
  « PRLV SEPA EDF », alors la proposition « Edf » suggère le tiers existant « EDF » (R-TIE-32).
- Étant donné la proposition « Carrefour Meylan » dont une transaction est rapprochée, quand je la valide en
  créant le tiers « Carrefour Meylan » avec la règle, alors les trois transactions ont ce tiers, leurs
  catégories sont inchangées, et la règle « Tiers : Carrefour Meylan » affecte ce tiers à l'import d'une ligne
  « CB CARREFOUR MEYLAN 20/04 » (R-TIE-33, R-TIE-34).
- Étant donné la proposition « Frais Tenue Compte », quand je l'ignore, alors elle n'est plus proposée, même
  après l'import d'une nouvelle ligne de même clé ; quand je la rétablis, elle est de nouveau proposée
  (R-TIE-35).

### TIE-06 — Critères d'acceptation

- Étant donné un tiers, quand je saisis le code postal « 38240 » sans ville, alors l'enregistrement est refusé
  (R-TIE-23).
- Étant donné la recherche d'adresse activée, quand je choisis la proposition « 8 Boulevard du Port 80000
  Amiens », alors le tiers a pour ville « Amiens », pour pays FR et des coordonnées (R-TIE-26).
- Étant donné un tiers géocodé, quand je corrige son code postal à la main, alors ses coordonnées sont
  effacées (R-TIE-25).
- Étant donné la recherche d'adresse désactivée, quand j'appelle la recherche, alors elle est refusée et
  aucune requête ne part vers la BAN ni vers Photon (R-TIE-26).
- Étant donné la recherche d'adresse activée et le pays Allemagne, quand je choisis la proposition
  « Unter den Linden 77, 10117 Berlin », alors le tiers a pour ligne « Unter den Linden 77 », pour ville
  « Berlin », pour pays DE et des coordonnées ; la recherche est partie vers Photon, restreinte à DE
  (R-TIE-26).

### TIE-07 — Critères d'acceptation

- Étant donné en mars −40 € chez « Boulangerie » (Meylan, 38240), −60 € chez « Carrefour Meylan » (MEYLAN,
  38240), −30 € chez « Fnac » (Grenoble, 38000) et −10 € chez « Amazon » (sans adresse), alors par ville :
  Meylan = 100 € (2 tiers), Grenoble = 30 €, Sans adresse = 10 € ; par département : Isère (38) = 130 €
  (R-TIE-24, R-TIE-28).
- Étant donné une adresse en Corse « 20090 Ajaccio », alors son département est 2A (R-TIE-24).
