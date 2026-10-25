# ADR-0023 — Jetons d'accès en lecture seule pour une application tierce

- **Statut** : Accepté
- **Date** : 2026-10-25
- **Concerne** : ADR-0006, ADR-0015, modèle de données (authentification), architecture ; Foodflow (ADR-0010, spec 08)

## Contexte

Foodflow, l'application de gestion alimentaire du foyer, veut lire dans Cashflow le budget courses du mois
et les transactions à rapprocher d'une session de courses (ADR-0010 et spec 08 de Foodflow). Les appels
partent du serveur Foodflow, sur le réseau local.

Cashflow n'offre qu'une session par cookie, ouverte avec le mot de passe (ADR-0015) : rien n'est prévu pour
qu'une autre application l'interroge. Lui confier le mot de passe donnerait un accès complet, en écriture,
et le moindre changement de mot de passe casserait le lien.

## Options envisagées

1. **Partager le mot de passe** — ❌ accès complet, couplé au mot de passe de l'utilisateur.
2. **Jeton d'accès complet** — ✅ simple / ❌ un jeton divulgué permet de tout modifier.
3. **Jeton en lecture seule, limité à une liste de routes fixée dans le code** — ✅ un jeton divulgué
   n'expose que ce dont Foodflow a besoin / ❌ ajouter une route demande une modification de Cashflow.
4. **Droits portés par chaque jeton** (`budget:read`…) — ✅ souple / ❌ prématuré avec un seul client.

Pour la gestion des jetons : un écran dans les Réglages (E9), ou des sous-commandes du binaire, comme
`set-password`. Les sous-commandes suffisent : un jeton se crée une fois, lors de la mise en place du lien,
par la personne qui administre le serveur.

## Décision

Option 3, gérée en ligne de commande (décision du propriétaire, 2026-10-25).

- **Création** : `cashflow token create <nom>` affiche le jeton **une seule fois** sur la sortie standard.
  Seule son empreinte SHA-256 est stockée (table `access_token`) : le jeton fait 256 bits aléatoires, un
  hachage lent comme argon2 n'apporte rien. Le jeton est préfixé `cf_` pour être reconnaissable (dans une
  configuration ou dans un outil de recherche de secrets).
- **Liste** : `cashflow token list` affiche le nom, la date de création et la date de dernier usage.
- **Révocation** : `cashflow token revoke <nom>` supprime le jeton.
- **Usage** : en-tête `Authorization: Bearer <jeton>`. Quand l'en-tête est présent, le cookie de session est
  ignoré.
- **Droits** : lecture seule, sur une liste de routes fixée dans le code :
  `GET /api/v1/categories`, `GET /api/v1/budget`, `GET /api/v1/transactions`,
  `GET /api/v1/transactions/{id}`, `GET /api/v1/payees`, `GET /api/v1/accounts`.
  Toute autre route répond **403** ; un jeton inconnu ou révoqué répond **401**. L'IBAN en clair n'est pas
  accessible avec un jeton.
- Le jeton **n'expire pas** ; le dernier usage est noté au plus une fois par heure, comme pour les sessions.
- `set-password` et « se déconnecter de tous les appareils » ferment les sessions, **pas** les jetons : ils
  appartiennent à une application, pas à un appareil.
- Le jeton référence l'utilisateur (ADR-0006), pour le jour où il y aura plusieurs membres. Comme les
  sessions, les jetons ne sont pas exportés (ADR-0008).
- Les routes autorisées deviennent un **contrat stable** pour Foodflow : leur évolution incompatible passe
  par une nouvelle version de l'API (ADR-0016).

## Conséquences

- Foodflow n'a besoin que de l'adresse de Cashflow et d'un jeton, dans sa configuration.
- En V1, le serveur est en HTTP (ADR-0015) : le jeton circule en clair sur le réseau local, comme le cookie
  de session. Le passage au HTTPS (Q15) protégera les deux.
- Ouvrir une nouvelle route à Foodflow demande une modification de Cashflow (la liste, et cet ADR).
- Un écran de gestion des jetons dans les Réglages reste possible plus tard, au-dessus des mêmes services.
