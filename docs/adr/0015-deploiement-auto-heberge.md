# ADR-0015 — Déploiement auto-hébergé sur le réseau local

- **Statut** : Accepté ; partie « HTTPS sur le réseau local » remplacée par ADR-0020
- **Date** : 2026-10-20
- **Concerne** : architecture, ADR-0006, ADR-0008, ADR-0009

## Contexte

L'application tourne sur un petit serveur personnel. Elle est utilisée depuis un ordinateur et un
téléphone, **uniquement sur le réseau local** (choix du 2026-10-20 : ni VPN, ni exposition sur Internet).
Les données sont bancaires. D'autres appareils partagent le réseau domestique (invités, objets connectés).

## Décision

### Empaquetage
- **Un seul binaire** `cashflow` qui sert l'API **et** les fichiers statiques de l'interface. Il dispose
  de sous-commandes : `serve`, `migrate`, `seed`, `export`, `import-export`, `backup`, `set-password`.
- **Image Docker** multi-étapes (build de l'interface, build Rust, image finale minimale non-root),
  publiée pour `linux/amd64` (serveur cible en x86-64) ; `linux/arm64` reste possible sans changement de code.
- Un volume `/data` contient tout l'état : `cashflow.db`, `imports/` (fichiers source), `backups/`.
- Un `compose.yaml` d'exemple : service `cashflow` + service `caddy`.

### HTTPS sur le réseau local
Le navigateur exige un contexte sécurisé pour installer une PWA et pour certaines API. **Caddy** sert
de reverse proxy TLS devant l'application, avec deux modes documentés :
1. **Domaine personnel + certificat Let's Encrypt par challenge DNS** (recommandé) : un sous-domaine
   (`cashflow.mondomaine.fr`) pointe vers l'IP **locale** du serveur. Le certificat est obtenu par DNS,
   sans ouvrir aucun port sur Internet. Rien n'est joignable de l'extérieur.
2. **Autorité de certification interne de Caddy** : pas de domaine nécessaire, mais il faut installer
   le certificat racine sur chaque appareil.

L'application elle-même écoute en HTTP sur une interface interne au réseau Docker et n'est pas publiée
directement.

### Authentification (précise ADR-0006)
- **Mono-utilisateur, mot de passe obligatoire**, même sur le réseau local : défini par
  `cashflow set-password`, haché en argon2id.
- Session par cookie `HttpOnly`, `Secure`, `SameSite=Strict`, d'une durée de 30 jours glissants par
  appareil. Bouton « se déconnecter de tous les appareils ».
- Limitation des tentatives de connexion (backoff exponentiel par IP).
- Pas de gestion de comptes ni de récupération par e-mail : la récupération se fait par `set-password`
  en ligne de commande sur le serveur.

### Sauvegardes (précise ADR-0008)
- **Sauvegarde automatique quotidienne** par le serveur (`VACUUM INTO` vers `backups/`, horodatée),
  ainsi qu'avant chaque migration. Rétention : 7 quotidiennes, 4 hebdomadaires, 12 mensuelles (réglable).
- La copie **hors du serveur** (disque externe, NAS, stockage chiffré distant) relève de l'administration
  du serveur. Elle est documentée, pas implémentée.

### Notifications
Uniquement **dans l'application** (centre de notifications) : le port `Notifier` a un seul adaptateur
qui écrit en base. Les notifications push (ntfy, Web Push) sont reportées en V2.

## Conséquences

- Hors du réseau local, l'application est inaccessible : c'est voulu. Ajouter plus tard un VPN
  (Tailscale, WireGuard) ne demanderait aucun changement à l'application.
- L'OCR optionnel (ADR-0007) reste le **seul flux sortant** de l'application.
- Le chiffrement au repos repose sur le chiffrement du disque du serveur (LUKS ou équivalent),
  documenté dans le guide d'installation.
- Le serveur exécute des tâches de fond légères (sauvegardes) : une boucle `tokio` dans `serve`, sans
  ordonnanceur externe.
