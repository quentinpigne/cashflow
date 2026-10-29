# ADR-0015 — Déploiement auto-hébergé sur le réseau local

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : architecture, ADR-0006, ADR-0008, ADR-0009, ADR-0018, ADR-0023, ADR-0024

## Contexte

L'application tourne sur un petit serveur personnel. Elle est utilisée depuis un ordinateur et un
téléphone, **uniquement sur le réseau local** (choix du 2026-10-20 : ni VPN, ni exposition sur Internet).
Les données sont bancaires. D'autres appareils partagent le réseau domestique (invités, objets connectés).

## Décision

### Empaquetage
- **Un seul binaire** `cashflow` qui sert l'API **et** les fichiers statiques de l'interface. Il dispose
  de sous-commandes : `serve`, `migrate`, `seed`, `export`, `import-export`, `backup`, `set-password`, `token` (jetons d'accès, ADR-0023).
- **Image Docker** multi-étapes (build de l'interface, build Rust, image finale minimale non-root),
  publiée pour `linux/amd64` (serveur cible en x86-64) ; `linux/arm64` reste possible sans changement de code.
- Un volume `/data` contient tout l'état : `cashflow.db`, `imports/` (fichiers source), `documents/`
  (bibliothèque de documents, ADR-0024), `backups/`.
- Un `compose.yaml` : un seul service, `cashflow`, publié sur le réseau local.

### HTTP en V1, HTTPS par axum ensuite
Options envisagées : **Caddy** en reverse proxy TLS devant l'application (certificats gérés
automatiquement, mais un conteneur et une configuration de plus, et un certificat racine à installer sur
chaque appareil faute de domaine), ou **HTTP simple en V1 puis TLS directement dans axum**. La seconde est
retenue :
- **V1** : l'application écoute en **HTTP**, publiée directement sur le réseau local. Pas de reverse proxy.
- **Plus tard** : le binaire sert lui-même le HTTPS (axum + `rustls`), avec un certificat fourni par
  configuration. Le mode d'obtention du certificat (domaine personnel avec challenge DNS, ou autorité
  interne) sera tranché à ce moment-là (Q15).

### Authentification (précise ADR-0006)
- **Mono-utilisateur, mot de passe obligatoire**, même sur le réseau local : défini par
  `cashflow set-password`, haché en argon2id.
- Session par cookie `HttpOnly`, `SameSite=Strict`, et `Secure` dès que le serveur sert du HTTPS, d'une
  durée de 30 jours glissants par appareil. Bouton « se déconnecter de tous les appareils ».
- Limitation des tentatives de connexion (backoff exponentiel par IP).
- Pas de gestion de comptes ni de récupération par e-mail : la récupération se fait par `set-password`
  en ligne de commande sur le serveur.
- Une application tierce (Foodflow) n'utilise pas de session : elle lit l'API avec un jeton d'accès en
  lecture seule (ADR-0023).

### Sauvegardes (précise ADR-0008)
- **Sauvegarde automatique quotidienne** par le serveur (`VACUUM INTO` vers `backups/`, horodatée),
  ainsi qu'avant chaque migration. Rétention : 7 quotidiennes, 4 hebdomadaires, 12 mensuelles (réglable).
- La copie **hors du serveur** (disque externe, NAS, stockage chiffré distant) relève de l'administration
  du serveur ; elle couvre tout le volume, documents compris (ADR-0024). Elle est documentée, pas implémentée.

### Notifications
Uniquement **dans l'application** (centre de notifications), écrites en base. Les notifications push (ntfy, Web Push) sont reportées en V2.

## Conséquences

- Hors du réseau local, l'application est inaccessible : c'est voulu. Ajouter plus tard un VPN
  (Tailscale, WireGuard) ne demanderait aucun changement à l'application.
- L'OCR optionnel (ADR-0007) reste le **seul flux sortant** de l'application.
- Risque accepté en V1 : sur le réseau local, le mot de passe et les données circulent **en clair** ;
  quiconque a accès au réseau (Wi-Fi compris) peut les intercepter.
- Pas d'installation en PWA ni de service worker en V1 (ils exigent HTTPS) : sur le téléphone,
  l'application s'ouvre dans le navigateur, un raccourci sur l'écran d'accueil reste possible.
- Le passage au HTTPS ne touche que le crate `server` et la configuration de déploiement.
- Le chiffrement au repos repose sur le chiffrement du disque du serveur (LUKS ou équivalent),
  documenté dans le guide d'installation.
- Le serveur exécute des tâches de fond légères (sauvegardes) : une boucle `tokio` dans `serve`, sans
  ordonnanceur externe.
