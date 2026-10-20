# ADR-0020 — HTTP simple en V1, HTTPS par axum ensuite

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : remplace la partie « HTTPS sur le réseau local » de l'ADR-0015 ; ADR-0019, Q15

## Contexte

L'ADR-0015 plaçait Caddy devant l'application pour servir l'interface en HTTPS sur le réseau local.
Le propriétaire du projet ne veut pas de ce composant supplémentaire pour la V1 : l'application
n'est joignable que depuis son réseau privé.

## Options envisagées

1. **Caddy en reverse proxy TLS** (ADR-0015). ✅ Certificats gérés automatiquement. ❌ Un conteneur et
   une configuration de plus ; certificat racine à installer sur chaque appareil sans domaine.
2. **HTTP simple en V1, puis TLS directement dans axum** (`rustls`). ✅ Rien à ajouter au déploiement en
   V1 ; plus tard, le binaire reste seul. ❌ En V1, trafic en clair sur le réseau local et pas de PWA.

## Décision

Option 2.

- **V1** : l'application écoute en **HTTP**, publiée directement sur le réseau local par
  `compose.yaml`. Pas de reverse proxy.
- **Plus tard** : le binaire servira lui-même le HTTPS (axum + `rustls`), avec un certificat fourni
  par configuration. Le mode d'obtention du certificat (domaine personnel avec challenge DNS, ou
  autorité interne) sera tranché à ce moment-là, dans un nouvel ADR.

## Conséquences

- Risque accepté : sur le réseau local, le mot de passe et les données circulent **en clair**.
  Quiconque a accès au réseau (Wi-Fi compris) peut les intercepter.
- Le cookie de session reste `HttpOnly` et `SameSite=Strict`, mais ne peut pas être `Secure` en V1. Le
  drapeau sera activé automatiquement quand le serveur servira du HTTPS.
- Pas d'installation en PWA ni de service worker en V1 : sur le téléphone, l'application s'ouvre dans le
  navigateur (un raccourci sur l'écran d'accueil reste possible).
- Le passage au HTTPS ne touche que le crate `server` et la configuration de déploiement.
