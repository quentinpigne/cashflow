# Installation et déploiement

Cashflow tourne dans **un seul environnement**, sur le serveur personnel (ADR-0015, ADR-0018) : une image
Docker, un dossier de l'hôte monté sur `/data` qui contient toutes les données, en HTTP sur le réseau local pour la V1.

L'image est **construite par la CI** et publiée sur `ghcr.io/quentinpigne/cashflow`, étiquetée par son commit
(et `latest`), seulement pour un push sur `main` dont tous les contrôles sont passés. Le serveur ne construit
rien : il télécharge l'image.

## Prérequis sur le serveur

- Docker avec le plugin compose, et Git.
- L'accès à l'image : rien à faire si le paquet `cashflow` est public sur GitHub ; s'il est privé,
  `docker login ghcr.io -u <utilisateur>` une fois, avec un jeton (classic) limité à `read:packages`.
- Le **disque chiffré** (LUKS ou équivalent) : c'est le chiffrement au repos retenu pour la base (Q11,
  ADR-0015). À vérifier **avant d'y saisir de vraies données**.
- Le port 3000 accessible depuis le réseau local seulement (pas de redirection sur la box).

## Premier démarrage

Les données vivent dans un dossier de l'hôte, désigné par `CASHFLOW_DATA_PATH` dans `deploy/.env` (non
versionné). Le conteneur tourne avec l'uid **10001** (utilisateur `cashflow` de l'image) : le dossier doit lui
appartenir, sinon l'application ne peut pas y écrire.

```sh
git clone <dépôt> cashflow && cd cashflow
sudo mkdir -p /srv/cashflow && sudo chown 10001:10001 /srv/cashflow   # sur le disque chiffré
echo 'CASHFLOW_DATA_PATH=/srv/cashflow' > deploy/.env
deploy/deploy.sh                                             # télécharge l'image et lance l'application
docker compose -f deploy/compose.yaml exec cashflow cashflow set-password
```

L'application répond sur `http://<adresse-du-serveur>:3000`. Les données initiales (établissements,
catégories par défaut, barèmes réglementés) sont chargées au premier démarrage.

## Mettre à jour

Une fois la CI du commit terminée et verte (sinon son image n'existe pas, et le script s'arrête) :

```sh
deploy/deploy.sh
```

Le script récupère la version (`git pull`), télécharge l'image du commit
(`ghcr.io/quentinpigne/cashflow:<commit>`), **sauvegarde la base**, puis relance. Les migrations de schéma
s'appliquent au démarrage, après une sauvegarde automatique supplémentaire. Le journal
`deploy/deployments.log` garde la liste des versions déployées.

## Revenir en arrière

1. Relancer la version précédente, dont l'image reste sur le registre :

   ```sh
   tail deploy/deployments.log          # repérer le commit précédent
   deploy/deploy.sh <commit-précédent>
   ```

2. **Si la version annulée contenait une migration**, restaurer aussi la sauvegarde prise juste avant le
   déploiement (`cashflow-<horodatage>-pre-deploy.db`) :

   ```sh
   docker compose -f deploy/compose.yaml stop
   docker compose -f deploy/compose.yaml run --rm --entrypoint sh cashflow -c \
     'ls /data/backups && cp /data/backups/<fichier> /data/cashflow.db && rm -f /data/cashflow.db-wal /data/cashflow.db-shm'
   docker compose -f deploy/compose.yaml start
   ```

## Sauvegardes

- **Automatiques**, dans le dossier de données (`$CASHFLOW_DATA_PATH/backups`, `/data/backups` dans le
  conteneur) : une par jour (rétention 7 jours, 4 semaines,
  12 mois), une avant chaque migration et une avant chaque déploiement (ADR-0015).
- **Manuelle** : `docker compose -f deploy/compose.yaml exec cashflow cashflow backup`.
- **Hors du serveur** : à organiser côté serveur (disque externe, NAS, stockage chiffré distant), par
  exemple une copie quotidienne des sauvegardes de la base et des documents (fiches de paie), qui n'existent
  qu'en un exemplaire dans le dossier de données (ADR-0024) :

  ```sh
  . deploy/.env
  sudo cp -n "$CASHFLOW_DATA_PATH"/backups/*.db /mnt/sauvegardes/
  sudo mkdir -p /mnt/sauvegardes/documents && sudo cp -Rn "$CASHFLOW_DATA_PATH"/documents/. /mnt/sauvegardes/documents/
  ```

- **Documents** : un document supprimé reste 400 jours dans `/data/documents/trash/`, préfixé de sa date de
  suppression ; après la restauration d'une sauvegarde, on le remet à sa place sous son identifiant
  (ADR-0024).

- **Export ouvert** (ADR-0008) : Réglages › Export, ou
  `docker compose -f deploy/compose.yaml exec cashflow cashflow export > export.json`.

## Commandes utiles

| Besoin | Commande |
|---|---|
| Journaux | `docker compose -f deploy/compose.yaml logs -f` |
| Changer le mot de passe | `docker compose -f deploy/compose.yaml exec cashflow cashflow set-password` |
| Jeton d'accès pour Foodflow (affiché une seule fois, ADR-0023) | `docker compose -f deploy/compose.yaml exec cashflow cashflow token create foodflow` |
| Lister / révoquer les jetons | `… exec cashflow cashflow token list` ; `… exec cashflow cashflow token revoke foodflow` |
| Instance d'essai jetable (données fictives, port 3001) | `docker run --rm -p 3001:3000 --entrypoint sh ghcr.io/quentinpigne/cashflow -c 'cashflow demo && cashflow serve'` (sans le compose, pour ne pas toucher au dossier de données) |
