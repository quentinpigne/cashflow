#!/bin/sh
# Déploiement sur le serveur (ADR-0018) : récupère la version, sauvegarde la base, reconstruit et relance.
# Usage : deploy/deploy.sh            déploie le dernier commit de la branche courante
#         deploy/deploy.sh <commit>   redéploie une version déjà construite (retour arrière)
set -eu
cd "$(dirname "$0")/.."
COMPOSE="docker compose -f deploy/compose.yaml"

if [ $# -eq 0 ]; then
  git pull --ff-only
  VERSION=$(git rev-parse --short HEAD)
  BUILD=1
else
  VERSION=$1
  BUILD=0
  docker image inspect "cashflow:$VERSION" > /dev/null || { echo "Image cashflow:$VERSION introuvable." >&2; exit 1; }
fi
export CASHFLOW_VERSION="$VERSION"

# Sauvegarde avant déploiement, si l'application tourne (ADR-0018).
if [ -n "$($COMPOSE ps --status running -q cashflow 2>/dev/null)" ]; then
  echo "Sauvegarde avant déploiement…"
  $COMPOSE exec -T cashflow cashflow backup --reason pre-deploy
fi

if [ "$BUILD" -eq 1 ]; then
  echo "Construction de cashflow:$VERSION…"
  $COMPOSE build
fi

$COMPOSE up -d
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $VERSION" >> deploy/deployments.log
echo "Cashflow $VERSION déployé. Versions précédentes : deploy/deployments.log"
