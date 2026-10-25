# ADR-0008 — Export complet dans un format ouvert

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : architecture, roadmap

## Contexte

Les données financières personnelles s'accumulent sur des années. Elles ne doivent dépendre ni de
la survie de l'application, ni d'une stack précise : on vient d'en changer une fois.

## Décision

- **Export complet** (MVP) : toutes les entités, au format JSON versionné (`schema_version`), avec
  les montants en centimes et les dates en ISO 8601. Un export suivi d'un import doit restituer
  exactement les mêmes données (test aller-retour).
- **Fichiers source des imports** : au choix au moment de l'export, par une case « inclure les fichiers
  source » décochée par défaut. Cochée, l'export devient une archive ZIP qui contient le JSON et les
  fichiers, référencés depuis leur lot d'import, ainsi que les documents archivés (fiches de paie,
  ADR-0024). La restauration est alors intégrale.
- **Export tabulaire** (V1) : transactions et ventilations en CSV, pour un tableur.
- **Sauvegarde** : la base est un fichier unique, copiable à chaud via le mécanisme de sauvegarde du
  moteur. Une sauvegarde automatique horodatée est faite avant chaque migration de schéma.

## Conséquences

- Le format d'export est un contrat : il est documenté et versionné comme une API.
- Une future migration de stack pourra passer par l'export JSON.
