# ADR-0024 — Documents archivés sur le disque

- **Statut** : Accepté
- **Date** : 2026-10-25
- **Concerne** : spec 10 (SAL-09), spec 02 (TRX-11), modèle de données, ADR-0008, ADR-0015

## Contexte

On veut archiver ses **fiches de paie** dans l'application, rattachées à un emploi, pour les consulter des
années plus tard (SAL-09). Les **pièces jointes** des transactions (TRX-11, V2) poseront la même question :
où ranger des fichiers envoyés par l'utilisateur, PDF ou images, de quelques centaines de Ko chacun.

Une archive n'a de valeur que si elle survit : à une suppression par erreur, à une migration ratée, à une
restauration de sauvegarde. Ces fichiers sont aussi sensibles (numéro de sécurité sociale, adresse, parfois
l'IBAN). Les fichiers source des imports sont déjà rangés sur le disque, dans `/data/imports` (R-IMP-21),
mais la sauvegarde quotidienne ne copie que la base.

## Options envisagées

1. **Contenu en BLOB dans la base** — ✅ couvert d'office par la sauvegarde, la restauration et l'export ;
   écriture atomique / ❌ la base grossit et chaque sauvegarde quotidienne recopie toute l'archive.
2. **Fichiers sur le disque, référencés par la base** — ✅ base légère, fichiers lisibles hors de
   l'application, même principe que les imports / ❌ la sauvegarde et l'export doivent tenir compte du
   dossier ; écriture en deux temps (fichier, puis ligne).
3. **Table propre aux fiches de paie, ou entité commune** — une entité `Document` générique, que les fiches
   de paie référencent et que TRX-11 réutilisera, coûte à peine plus qu'une table dédiée.

## Décision

Option 2, avec une entité **`Document`** commune (décision du propriétaire, 2026-10-25).

- **Stockage** : chaque document est un fichier `/data/documents/<id>`, nommé par son identifiant, sans
  extension. La base garde le nom d'origine, le type, la taille et l'empreinte SHA-256. Le contenu est
  **immuable** : on ne remplace pas un document, on en envoie un autre.
- **Types acceptés** : PDF, JPEG, PNG, d'au plus **10 Mo**. Le type est reconnu à la signature du contenu,
  pas à l'extension ni au type annoncé par le navigateur.
- **Écriture** : le fichier est écrit sous un nom temporaire, puis renommé une fois la ligne enregistrée ;
  un échec ne laisse ni ligne sans fichier ni fichier visible sans ligne.
- **Suppression différée** : supprimer un document retire sa ligne et déplace le fichier dans
  `/data/documents/trash/`, préfixé de la date de suppression. Le fichier est effacé après **400 jours**,
  plus que la rétention la plus longue des sauvegardes (12 mensuelles, ADR-0015). Toute sauvegarde de la
  base encore conservée reste ainsi **restaurable avec ses documents**, sans copier l'archive à chaque
  sauvegarde. La purge a lieu avec la sauvegarde quotidienne.
- **Consultation** : le document est servi avec son type, `Content-Disposition` (`inline` pour l'afficher,
  `attachment` pour le télécharger), `X-Content-Type-Options: nosniff` et `Content-Security-Policy: sandbox`,
  pour qu'un fichier piégé ne puisse rien exécuter dans l'origine de l'application.
- **Export** (précise ADR-0008) : l'export JSON contient les métadonnées des documents ; leur contenu entre
  dans l'archive ZIP de l'option « inclure les fichiers », avec les fichiers source des imports.
- Un document n'existe que rattaché : une fiche de paie (SAL-09), plus tard une transaction (TRX-11). Il est
  supprimé avec ce qui le porte.

## Conséquences

- Le volume `/data` contient un dossier de plus, `documents/`. La copie **hors du serveur** (ADR-0015) doit
  couvrir tout le volume, pas seulement `backups/` : un document n'existe qu'en un exemplaire sur le disque.
- Restaurer une sauvegarde de la base ne touche pas `documents/` ; un document supprimé depuis se retrouve
  dans `documents/trash/` et se remet en place à la main.
- Ces fichiers rendent le chiffrement au repos (Q11) plus important : ils contiennent des données que la
  base ne contient pas (numéro de sécurité sociale).
- TRX-11 n'aura qu'à rattacher des documents aux transactions.
