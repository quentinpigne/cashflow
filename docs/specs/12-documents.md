# Spec 12 — Documents

Statut : **Revue** (2026-10-29)
Modèle : [Document, DocumentLink](../modele-de-donnees.md#documents--spec-12)
Décision : [ADR-0024](../adr/0024-documents-sur-le-disque.md) (bibliothèque de documents sur le disque)

## Concepts

La **bibliothèque de documents** archive les papiers de la vie financière : relevés de compte, fiches de
paie, contrats (banque, travail, assurance, bail, achat ou vente), tickets de caisse, factures, documents
fiscaux, attestations, courriers. Elle répond à deux besoins : **garder** ces documents des années, et les
**retrouver** vite.

Un **document** (`Document`) est un fichier immuable (PDF, JPEG ou PNG) accompagné d'une description : un
**type**, un titre, une date, éventuellement une période et un mois, une note. Il est **rattaché** à zéro,
un ou plusieurs objets de l'application : un compte, un établissement, un emploi, une transaction, un bien,
un prêt, un tiers, un projet. La facture d'une voiture se rattache à la fois à la transaction qui l'a payée,
au bien « Voiture » et au tiers « Garage Dupont ».

```
Document  « Facture Clio »  (facture, 14 mars 2025)
 ├─ rattaché à  Transaction  −18 900,00 € du 14 mars 2025
 ├─ rattaché à  Bien         Voiture
 └─ rattaché à  Tiers        Garage Dupont
Document  « Avis d'imposition 2025 »  (document fiscal)   ← sans rattachement
```

Le **relevé de compte** et la **fiche de paie** sont des documents d'un type particulier, rattachés à un
compte ou à un emploi pour un mois : leurs vues (CPT-11, SAL-09) sont des vues de la bibliothèque.

Les fichiers source des imports (`/data/imports`, R-IMP-21) ne sont **pas** des documents : ce sont des
fichiers techniques, attachés à un lot d'import. Un relevé PDF importé est copié dans la bibliothèque
(R-IMP-28).

## Fichier

- **R-DOC-1** — Le fichier d'un document est un **PDF**, un **JPEG** ou un **PNG** d'au plus **10 Mo**, reconnu
  à son contenu et non à son extension ni au type annoncé par le navigateur. Un relevé de compte est un PDF.
- **R-DOC-2** — Le fichier est **immuable** : on modifie la description et les rattachements d'un document,
  jamais son fichier. Pour corriger un fichier, on envoie le bon, puis on supprime l'ancien.
- **R-DOC-3** — Un même fichier (même empreinte SHA-256) n'est archivé **qu'une fois** dans toute la
  bibliothèque. Un second envoi est refusé avec le titre du document existant, et propose de rattacher ce
  document à l'objet depuis lequel on l'envoyait.

## Description

- **R-DOC-4** — Un document a un **titre** (≤ 120 caractères), prérempli avec le nom du fichier sans son
  extension, et un **nom de fichier** d'origine, conservé pour le téléchargement.
- **R-DOC-5** — Le **type** (`DocumentKind`) est l'un de : relevé de compte, fiche de paie, contrat, ticket de
  caisse, facture, document fiscal, attestation, courrier, autre. Un document **sans type** est **à classer** :
  c'est l'état d'un document envoyé sans être décrit (R-DOC-14).
- **R-DOC-6** — La **date du document** est facultative : date du ticket, de la facture, de signature d'un
  contrat, d'émission d'un avis. La **période** (début, fin) est facultative, chaque borne indépendamment,
  le début avant la fin : période couverte par un relevé, durée d'un contrat (un CDI n'a pas de fin), validité
  d'une attestation ou d'une garantie.
- **R-DOC-7** — Le **mois** est requis pour un relevé de compte (mois du relevé) et une fiche de paie (mois de
  paie), et absent pour les autres types. Changer le type d'un document en relevé ou en fiche de paie demande
  ce mois (et le rattachement de R-DOC-10) ; le changer vers un autre type efface le mois.
- **R-DOC-8** — La **date de référence** d'un document sert au tri et au filtre par année : la date du
  document, à défaut le premier jour du mois, à défaut le début de la période, à défaut la date d'envoi.

## Rattachements

- **R-DOC-9** — Un document est rattaché à **zéro, un ou plusieurs** objets parmi : compte, établissement,
  emploi, transaction, bien, prêt, tiers, projet. Il est rattaché au plus une fois à un même objet. Le
  rattachement d'un document n'a aucun effet sur les montants : il n'a rien à voir avec le rattachement d'une
  ventilation (R-TRX-37).
- **R-DOC-10** — Un **relevé de compte** est rattaché à **un seul compte** et une **fiche de paie** à **un seul
  emploi**, en plus de ses autres rattachements éventuels. Un relevé ou une fiche dont le compte ou l'emploi a
  été supprimé (R-DOC-12) n'a plus ce rattachement : on peut le rattacher à un autre compte ou emploi, ou
  changer son type.
- **R-DOC-11** — Rattacher un document à une transaction `RECONCILED`, ou l'en détacher, est permis sans la
  déverrouiller : ni le montant, ni la date, ni la ventilation ne changent (comme le tiers, R-TIE-12).
- **R-DOC-12** — **Supprimer un objet** qui a des documents rattachés retire ces rattachements ; les
  documents **restent** dans la bibliothèque. La confirmation indique leur nombre et propose une case,
  décochée par défaut, « Supprimer aussi les N documents rattachés seulement à <objet> » ; un document
  rattaché à un autre objet n'est jamais supprimé ainsi. Cela vaut pour toute suppression : compte vide
  (R-CPT-7), emploi (R-SAL-6), bien (R-BIE-6), prêt (R-PRE-11), tiers (R-TIE-5), projet, transaction (y
  compris par l'annulation d'un import, R-IMP-17, où les documents sont toujours conservés).
- **R-DOC-13** — **Fusionner** un tiers dans un autre (R-TIE-7) reporte ses rattachements de documents sur le
  tiers conservé ; un document rattaché aux deux n'en garde qu'un.

## Envoi

- **R-DOC-14** — Depuis la **bibliothèque**, on envoie un ou plusieurs fichiers d'un coup : chacun devient un
  document à classer (sans type), sans rattachement. Un fichier refusé (R-DOC-1, R-DOC-3) n'empêche pas
  l'archivage des autres ; chaque refus est indiqué avec sa raison.
- **R-DOC-15** — Depuis la **fiche d'un objet**, le document envoyé est rattaché d'office à cet objet, avec un
  type proposé : relevé de compte depuis la section Relevés d'un compte, fiche de paie depuis la section
  Fiches de paie d'un emploi, ticket de caisse depuis une dépense, contrat depuis la section Documents d'un
  compte, d'un emploi ou d'un prêt, facture depuis un bien ; aucun ailleurs. Le type reste modifiable avant
  d'enregistrer. Le titre d'un relevé ou d'une fiche de paie est prérempli d'après l'objet et le mois :
  « Relevé Compte courant septembre 2026 », « Fiche de paie Acme août 2026 ».
- **R-DOC-16** — Depuis la fiche d'un objet, on peut aussi **rattacher un document existant**, choisi par la
  recherche (R-DOC-18).
- **R-DOC-17** — Un relevé PDF validé à l'import est archivé comme relevé de compte (R-IMP-28).

## Recherche

- **R-DOC-18** — La recherche par texte trouve un document par son titre, sa note, son nom de fichier, le nom
  des objets auxquels il est rattaché (compte, établissement, employeur, bien, prêt, tiers, projet, libellé de
  la transaction) et le **texte de son contenu** (R-DOC-19). Elle ignore la casse et les accents, et trouve un
  mot par son début (« assur » trouve « Assurance »).
- **R-DOC-19** — À l'envoi d'un PDF, le texte qu'il contient est extrait sur le serveur et indexé, sans
  service tiers (ADR-0007). Une image, ou un PDF scanné sans texte, n'a pas de contenu indexé : il ne se
  trouve que par sa description et ses rattachements, et l'interface le signale (« contenu non indexé »).
  Une extraction qui échoue n'empêche pas l'archivage. L'index est dérivé des fichiers : il se reconstruit
  à la demande (`cashflow documents reindex`).
- **R-DOC-20** — La recherche se combine avec des **filtres** : type (dont « à classer »), année de la date de
  référence, objet rattaché (un objet précis, ou une nature d'objet : « rattachés à un prêt »), et « sans
  rattachement ». Sans texte, les documents sont triés par date de référence décroissante ; avec un texte,
  par pertinence, puis par date de référence.

## Consultation et suppression

- **R-DOC-21** — Un document s'**affiche** dans l'application (aperçu du PDF ou de l'image), s'ouvre dans un
  nouvel onglet ou se **télécharge** sous son nom de fichier d'origine.
- **R-DOC-22** — **Supprimer un document** retire ses rattachements et le retire de la bibliothèque, après une
  confirmation qui liste ses rattachements. Le fichier est conservé 400 jours dans la corbeille du serveur,
  pour qu'une sauvegarde restaurée retrouve ses documents (ADR-0024).

## Features

| ID | Priorité | Feature |
|---|---|---|
| DOC-01 | V1 | Bibliothèque : envoyer (un ou plusieurs fichiers), décrire, consulter, supprimer des documents |
| DOC-02 | V1 | Rattacher un document à un ou plusieurs objets ; section Documents sur les fiches |
| DOC-03 | V1 | Rechercher : texte (description, rattachements, contenu des PDF) et filtres |
| DOC-04 | V1 | Justificatifs d'une transaction, photographiés depuis le téléphone (remplace TRX-11) |
| DOC-05 | V2 | Échéances : alerte avant la fin d'un contrat ou d'une garantie, avec un préavis |
| DOC-06 | V2 | Reconnaissance du texte (OCR) des images et des PDF scannés, pour la recherche (ADR-0007) |

Exemple commun : un compte courant LCL avec ses relevés PDF archivés depuis l'import ; un emploi chez Acme avec
ses fiches de paie ; un bien « Voiture » acheté le 14 mars 2025 au tiers « Garage Dupont » pour 18 900 €.

### DOC-01 — Bibliothèque

- Je dépose dans la bibliothèque quatre fichiers : un contrat d'assurance habitation en PDF, deux photos de
  tickets et un document Word renommé en `.pdf`. Les trois premiers sont archivés « à classer » ; le dernier
  est refusé : « Type de fichier non accepté (PDF, JPEG ou PNG). »
- J'ouvre le contrat, je choisis le type « Contrat », le titre « Assurance habitation MAIF », la période du
  1er janvier 2026 sans fin : il n'est plus à classer.
- Je redépose le même contrat : il est refusé, « Ce fichier est déjà archivé (Assurance habitation MAIF). »
- Je supprime un ticket : il disparaît de la bibliothèque et de toutes les fiches où il était rattaché.

### DOC-02 — Rattachements

- Sur la facture de la Clio, j'ajoute trois rattachements : la transaction de −18 900 €, le bien « Voiture »
  et le tiers « Garage Dupont ». La facture apparaît dans la section Documents des trois fiches.
- Je supprime le bien « Voiture » sans cocher la case : la facture reste, rattachée à la transaction et au
  tiers. En cochant la case, elle reste aussi, puisqu'elle est rattachée à d'autres objets.
- Je supprime l'emploi chez Acme en cochant « Supprimer aussi les 14 documents rattachés seulement à Acme » :
  les 14 fiches de paie sont supprimées ; le contrat de travail, rattaché aussi au tiers « Acme », reste.
- Une fiche de paie ne peut pas être enregistrée sans emploi ni sans mois de paie (R-DOC-10, R-DOC-7).

### DOC-03 — Recherche

- « maif » trouve le contrat « Assurance habitation MAIF », et un courrier sans titre parlant de la MAIF dans
  son texte (PDF avec texte).
- « garage » trouve la facture de la Clio par son tiers.
- Le filtre « Fiche de paie » et l'année 2025 donnent les fiches de paie de 2025, tous emplois confondus.
- Une photo de ticket sans titre parlant n'est trouvée que par ses rattachements ; sa ligne porte « contenu
  non indexé ».

### DOC-04 — Justificatifs d'une transaction

- Sur le téléphone, depuis la fiche d'une dépense chez Carrefour, « Ajouter un justificatif » ouvre l'appareil
  photo ; la photo devient un ticket de caisse rattaché à la transaction (et au tiers de la transaction s'il y
  en a un, proposé coché).
- Sur une transaction rapprochée, j'ajoute la facture sans déverrouiller la transaction (R-DOC-11).
- La liste des transactions (E14) peut se filtrer sur « avec justificatif » ou « sans justificatif ».
