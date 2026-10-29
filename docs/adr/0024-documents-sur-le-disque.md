# ADR-0024 — Bibliothèque de documents sur le disque

- **Statut** : Accepté
- **Date** : 2026-10-25 (bibliothèque, rattachements multiples et recherche : 2026-10-29)
- **Concerne** : spec 12, spec 10 (SAL-09), spec 01 (CPT-11), spec 03 (R-IMP-28), spec 02 (TRX-11), modèle de
  données, export, ADR-0007, ADR-0008, ADR-0015

## Contexte

On veut archiver dans l'application les papiers de sa vie financière, pour les consulter des années plus
tard : d'abord les **fiches de paie** d'un emploi (SAL-09, 2026-10-25) et les **relevés** d'un compte (CPT-11,
2026-10-28), puis (2026-10-29) tous les autres : contrats de banque, de travail, d'assurance, d'achat ou de
vente, tickets de caisse, factures, documents fiscaux, attestations. Ce sont des PDF ou des images de
quelques centaines de Ko.

Un même document concerne souvent plusieurs choses : la facture d'une voiture concerne la transaction qui l'a
payée, le bien et le garage ; un contrat de prêt concerne le prêt, le bien financé et la banque. Et un
document archivé n'a de valeur que si on le **retrouve** : par son type, sa date, ce qu'il concerne ou ce
qu'il contient.

Une archive doit aussi survivre : à une suppression par erreur, à une migration ratée, à une restauration
de sauvegarde. Ces fichiers sont sensibles (numéro de sécurité sociale, adresse, parfois l'IBAN). Les
fichiers source des imports sont déjà rangés sur le disque, dans `/data/imports` (R-IMP-21), mais la
sauvegarde quotidienne ne copie que la base.

## Options envisagées

**Stockage du contenu**

1. **BLOB dans la base** — ✅ couvert d'office par la sauvegarde, la restauration et l'export ; écriture
   atomique / ❌ la base grossit et chaque sauvegarde quotidienne recopie toute l'archive.
2. **Fichiers sur le disque, référencés par la base** — ✅ base légère, fichiers lisibles hors de
   l'application, même principe que les imports / ❌ la sauvegarde et l'export doivent tenir compte du
   dossier ; écriture en deux temps (fichier, puis ligne).

**Forme du modèle**

3. **Une table par usage** (`payslip`, `bank_statement`, puis contrats, justificatifs…) qui pointe vers un
   `Document` — ✅ chaque usage garde ses champs / ❌ autant de tables et d'écrans que d'usages, deux façons
   de décrire un relevé, une recherche qui doit les parcourir toutes.
4. **Un `Document` typé, rattaché à des objets** — ✅ un seul concept, une seule recherche ; les relevés et
   fiches de paie deviennent des vues filtrées / ❌ quelques règles propres à un type (mois requis, un seul
   compte) passent dans le service plutôt que dans le schéma.

**Rattachements**

5. **Lien polymorphe** (`target_kind`, `target_id`) — ✅ une nouvelle cible ne change pas le schéma / ❌
   aucune clé étrangère : la base ne garantit plus qu'un rattachement désigne un objet existant.
6. **Une colonne par cible**, exactement une renseignée (`CHECK`) — ✅ clés étrangères vérifiées par SQLite,
   suppression en cascade du rattachement / ❌ une nouvelle cible demande une migration (recréation de la
   table, ADR-0017).

**Recherche**

7. **Métadonnées seules** (`LIKE`) — ✅ simple / ❌ un document mal nommé est perdu.
8. **Métadonnées et texte des PDF**, index plein texte FTS5 de SQLite — ✅ un contrat se retrouve par un mot
   de son contenu, sans service tiers ; FTS5 est compilé dans le SQLite embarqué par sqlx / ❌ un index de
   plus à tenir à jour ; les images n'ont pas de texte.
9. **Avec OCR des images** — ✅ les tickets se retrouvent aussi par leur contenu / ❌ un moteur OCR dans
   l'image Docker ou un service tiers (ADR-0007) ; reporté (DOC-06).

## Décision

Options **2**, **4**, **6** et **8** (décisions du propriétaire, 2026-10-25 et 2026-10-29).

- **Stockage** : chaque document est un fichier `/data/documents/<id>`, nommé par son identifiant, sans
  extension. La base garde le nom d'origine, le type de contenu, la taille et l'empreinte SHA-256. Le contenu
  est **immuable** : on ne remplace pas un document, on en envoie un autre.
- **Types acceptés** : PDF, JPEG, PNG, d'au plus **10 Mo**. Le type est reconnu à la signature du contenu,
  pas à l'extension ni au type annoncé par le navigateur.
- **Unicité** : une empreinte SHA-256 n'existe qu'une fois dans la bibliothèque (index unique). Un fichier
  déjà archivé ne crée pas de second document : on rattache l'existant.
- **Écriture** : le fichier est écrit sous un nom temporaire, puis renommé une fois la ligne enregistrée ;
  un échec ne laisse ni ligne sans fichier ni fichier visible sans ligne.
- **Document typé** : `Document` porte un type (`DocumentKind`, absent tant qu'il est « à classer »), un
  titre, une date, une période, un mois (relevés et fiches de paie seulement) et une note. Les tables
  `payslip` et `bank_statement` disparaissent : une migration reprend leurs lignes en documents de type
  `PAYSLIP` et `BANK_STATEMENT`, avec leur mois, leur période, leur note, le rattachement à leur emploi ou
  à leur compte, et un titre construit comme à l'envoi (R-DOC-15). Un fichier archivé deux fois (même
  empreinte, sous deux emplois ou deux comptes) ne garde que le plus ancien ; les fichiers des autres restent
  sur le disque, sans ligne. Le texte des documents repris est indexé au démarrage suivant.
- **Rattachements** : table `document_link`, une ligne par rattachement, une colonne de clé étrangère par
  cible (compte, établissement, emploi, transaction, bien, prêt, tiers, projet), exactement une renseignée.
  Supprimer l'objet supprime le rattachement (`ON DELETE CASCADE`), **jamais le document** : un document
  n'est supprimé que par une demande explicite, éventuellement proposée dans la confirmation de suppression
  de l'objet (R-DOC-12). Les règles propres à un type (un relevé a un seul compte, un mois) sont vérifiées par
  le domaine.
- **Recherche** : une table virtuelle FTS5 (`document_search`, tokeniseur `unicode61 remove_diacritics 2`,
  recherche par préfixe) indexe le titre, la note, le nom de fichier et le texte extrait des PDF ; le service
  la tient à jour dans la même transaction SQLite que le document. Les noms des objets rattachés ne sont pas
  indexés (ils changent quand on renomme l'objet) : la recherche les compare par une jointure. Le texte est
  extrait avec `pdf-extract`, déjà utilisé pour les relevés LCL, panique confinée ; un échec laisse le
  document sans contenu indexé. L'index est dérivé : il n'est ni exporté ni sauvegardé à part, et se
  reconstruit à partir des fichiers (`cashflow documents reindex`).
- **Suppression différée** : supprimer un document retire sa ligne et ses rattachements, et déplace le fichier
  dans `/data/documents/trash/`, préfixé de la date de suppression. Le fichier est effacé après **400 jours**,
  plus que la rétention la plus longue des sauvegardes (12 mensuelles, ADR-0015). Toute sauvegarde de la
  base encore conservée reste ainsi **restaurable avec ses documents**, sans copier l'archive à chaque
  sauvegarde. La purge a lieu avec la sauvegarde quotidienne.
- **Consultation** : le document est servi avec son type, `Content-Disposition` (`inline` pour l'afficher,
  `attachment` pour le télécharger), `X-Content-Type-Options: nosniff` et `Content-Security-Policy: sandbox`,
  pour qu'un fichier piégé ne puisse rien exécuter dans l'origine de l'application.
- **Export** (précise ADR-0008) : l'export JSON contient les tables `document` et `document_link` ; le contenu
  des documents entre dans l'archive ZIP de l'option « inclure les fichiers », avec les fichiers source des
  imports.
- **Fichiers d'import** (2026-10-28, confirmé le 2026-10-29) : le fichier source d'un import reste dans
  `/data/imports` (R-IMP-21), hors de la bibliothèque, avec sa propre vie (supprimable, effacé avec un lot
  abandonné). À la validation d'un import PDF, une **copie** devient un document de type relevé (R-IMP-28) :
  quelques centaines de Ko en double, contre deux cycles de vie indépendants et sans référence croisée. Les
  imports PDF validés avant le 2026-10-28 ont été repris une fois, au premier démarrage suivant, si leur fichier
  source existait encore.

## Conséquences

- Le volume `/data` contient un dossier de plus, `documents/`. La copie **hors du serveur** (ADR-0015) doit
  couvrir tout le volume, pas seulement `backups/` : un document n'existe qu'en un exemplaire sur le disque.
- Restaurer une sauvegarde de la base ne touche pas `documents/` ; un document supprimé depuis se retrouve
  dans `documents/trash/` et se remet en place à la main.
- Ces fichiers rendent le chiffrement au repos (Q11) plus important : ils contiennent des données que la
  base ne contient pas (numéro de sécurité sociale).
- Le texte extrait des PDF entre dans la base (index FTS5) : elle grossit de quelques Ko par document, et
  contient désormais une partie du contenu des documents. Comme la base est sauvegardée chaque jour, ce texte
  est copié dans chaque sauvegarde.
- Une nouvelle cible de rattachement (une récurrence, par exemple) demande une migration de `document_link`.
- Supprimer un objet ne supprime plus ses relevés ou ses fiches de paie : des documents sans rattachement
  peuvent s'accumuler, et la bibliothèque les montre (filtre « sans rattachement »).
- La reconnaissance du texte des images (DOC-06) reste à décider : moteur local ou service tiers explicite
  (ADR-0007).
- TRX-11 devient DOC-04 : les justificatifs d'une transaction sont des documents rattachés à la transaction.
