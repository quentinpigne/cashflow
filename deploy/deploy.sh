#!/bin/sh
# Déploiement sur le serveur (ADR-0018) : récupère la version, télécharge l'image publiée par la CI,
# sauvegarde la base et relance. Le serveur ne construit rien.
# Usage : deploy/deploy.sh            déploie le dernier commit de la branche courante
#         deploy/deploy.sh <commit>   redéploie une version précédente (retour arrière)
set -eu
cd "$(dirname "$0")/.."
COMPOSE="docker compose -f deploy/compose.yaml"

if [ $# -eq 0 ]; then
  git pull --ff-only
  VERSION=$(git rev-parse --short=7 HEAD)
else
  VERSION=$(git rev-parse --short=7 "$1^{commit}")
fi
export CASHFLOW_VERSION="$VERSION"

# La CI ne publie l'image d'un commit qu'une fois tous ses contrôles passés.
echo "Téléchargement de l'image $VERSION…"
$COMPOSE pull cashflow || {
  echo "Image $VERSION introuvable : la CI de ce commit est-elle terminée et verte ?" >&2
  exit 1
}

# Sauvegarde avant déploiement, si l'application tourne (ADR-0018).
if [ -n "$($COMPOSE ps --status running -q cashflow 2>/dev/null)" ]; then
  echo "Sauvegarde avant déploiement…"
  $COMPOSE exec -T cashflow cashflow backup --reason pre-deploy
fi

$COMPOSE up -d
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $VERSION" >> deploy/deployments.log
echo "Cashflow $VERSION déployé. Versions précédentes : deploy/deployments.log"
