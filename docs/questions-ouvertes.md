# Questions ouvertes

Statut : **vivant** — on retire une question dès qu'elle est tranchée, en indiquant où la réponse
a été consignée (ADR, spec).

## Bloquantes (phase 0)

| # | Question | Où | Impact |
|---|---|---|---|
| — | *Aucune.* Les ADR `Proposé` restent à relire (cf. roadmap, phase 0). | — | — |

## À trancher avant la phase concernée

| # | Question | Proposition par défaut | Où |
|---|---|---|---|
| Q4 | Quelles autres banques que LCL, et quels formats d'export proposent-elles ? | Recenser avant la phase 2 | spec 03, roadmap |
| Q11 | Chiffrement de la base au repos | Chiffrement du disque du serveur (ADR-0015) ; SQLCipher non retenu pour l'instant | ADR-0015 |
| Q15 | HTTPS local : domaine personnel avec challenge DNS, ou autorité de certification interne ? | Sans objet en V1 (HTTP simple). À trancher au passage au HTTPS servi par axum | ADR-0015 |

## Pour plus tard (après la V1)

| # | Question | Où |
|---|---|---|
| Q19 | Multi-membres : budgets, projets et scénarios sont-ils personnels, familiaux, ou les deux ? Que voit un membre `VIEWER` ? | ADR-0006 |

## Tranchées

| # | Question | Réponse | Où |
|---|---|---|---|
| Q5 | Conserver le fichier source de chaque import ? | Oui, supprimable | spec 03, R-IMP-21 |
| — | Formats LCL | CSV/Excel pour la période récente, PDF pour l'historique | spec 03, roadmap |
| — | Date conservée au pointage | Celle de l'utilisateur ; la date bancaire va dans `bank_date` | spec 02, R-TRX-7 |
| Q6 | Tolérance de date pour les correspondances d'import et de virement | ±5 jours, réglable | spec 02, spec 03 |
| Q10 | Suivi par support (fonds euros / UC) dès la V1 ? | Non, en V2 | ADR-0010 |
| Q7 | Périmètre de l'épargne de précaution | Drapeau par compte, prérempli (courants, espèces, livrets réglementés) | spec 01, R-CPT-13 |
| Q8 | Nombre de mois pour les dépenses moyennes | 12 | spec 08, R-TDB-8 |
| Q9 | Hypothèses de rendement par défaut des scénarios | Table de R-PRJ-31 | spec 07 |
| Q13 | Arrondi des intérêts | Cumul exact, arrondi annuel ; option par compte pour arrondir chaque quinzaine | spec 06, R-EPA-6 |
| Q12 | Seuil d'« ancienneté » d'une valorisation | 90 jours | spec 01, R-CPT-6 |
| — | Langue de la documentation | Français ; identifiants de code en anglais | README, glossaire |
| Q1 | Stack technique | Rust (axum, sqlx/SQLite) + SPA Angular / Angular Material | ADR-0009 |
| Q3 | Mode d'utilisation | Ordinateur et téléphone, serveur auto-hébergé, réseau local uniquement, notifications dans l'application | ADR-0015 |
| Q18 | REST ou GraphQL | REST + OpenAPI confirmé (un seul client, agrégats calculés côté serveur, upload de fichiers, erreurs typées) | ADR-0016 |
| Q17 | Base de données et accès | SQLite (fichier unique) + sqlx, sans ORM ; PostgreSQL, MariaDB et SeaORM écartés | ADR-0017 |
| Q16 | Stockage des identifiants | `TEXT` (UUID lisible au débogage ; volumétrie négligeable) | ADR-0017 |
| Q14 | Serveur | x86-64, déploiement Docker ; image `linux/amd64` (ARM non nécessaire) | ADR-0015 |
| Q2 | Modèle de budget | Lignes par catégorie (mensuelles/annuelles, report au choix) + projets + objectifs d'épargne | ADR-0014, spec 05 |
