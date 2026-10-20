# Installation et déploiement

Cashflow tourne dans **un seul environnement**, sur le serveur personnel (ADR-0015, ADR-0018) : une image
Docker, un volume qui contient toutes les données, en HTTP sur le réseau local pour la V1.

## Prérequis sur le serveur

- Docker avec le plugin compose, et Git.
- Le **disque chiffré** (LUKS ou équivalent) : c'est le chiffrement au repos retenu pour la base (Q11,
  ADR-0015). À vérifier **avant d'y saisir de vraies données**.
- Le port 3000 accessible depuis le réseau local seulement (pas de redirection sur la box).

## Premier démarrage

```sh
git clone <dépôt> cashflow && cd cashflow
deploy/deploy.sh                                             # construit l'image et lance l'application
docker compose -f deploy/compose.yaml exec cashflow cashflow set-password
```

L'application répond sur `http://<adresse-du-serveur>:3000`. Les données initiales (établissements,
catégories par défaut, barèmes réglementés) sont chargées au premier démarrage.

## Mettre à jour

Seulement un commit dont la CI est verte (ADR-0018) :

```sh
deploy/deploy.sh
```

Le script récupère la version (`git pull`), **sauvegarde la base**, construit l'image étiquetée par le
commit (`cashflow:<commit>`), puis relance. Les migrations de schéma s'appliquent au démarrage, après une
sauvegarde automatique supplémentaire. Le journal `deploy/deployments.log` garde la liste des versions
déployées.

## Revenir en arrière

1. Relancer la version précédente, dont l'image est conservée :

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

- **Automatiques**, dans le volume (`/data/backups`) : une par jour (rétention 7 jours, 4 semaines,
  12 mois), une avant chaque migration et une avant chaque déploiement (ADR-0015).
- **Manuelle** : `docker compose -f deploy/compose.yaml exec cashflow cashflow backup`.
- **Hors du serveur** : à organiser côté serveur (disque externe, NAS, stockage chiffré distant), par
  exemple une copie quotidienne de `/data/backups` :

  ```sh
  docker run --rm -v cashflow_data:/data:ro -v /mnt/sauvegardes:/out alpine \
    sh -c 'cp -n /data/backups/*.db /out/'
  ```

- **Export ouvert** (ADR-0008) : Réglages › Export, ou
  `docker compose -f deploy/compose.yaml exec cashflow cashflow export > export.json`.

## Commandes utiles

| Besoin | Commande |
|---|---|
| Journaux | `docker compose -f deploy/compose.yaml logs -f` |
| Changer le mot de passe | `docker compose -f deploy/compose.yaml exec cashflow cashflow set-password` |
| Instance d'essai jetable (données fictives, port 3001) | `docker compose -f deploy/compose.yaml -p cashflow-essai run --rm -p 3001:3000 --entrypoint sh cashflow -c 'cashflow demo && cashflow serve'` |
