# Format d'export

Statut : **Validée** (2026-10-20)

Format de l'export complet (ADR-0008), produit par `GET /api/v1/export` ou `cashflow export`, et relu par
`cashflow import-export` dans une base neuve. C'est un contrat : toute évolution incompatible change
`schema_version`.

## Structure

```json
{
  "format": "cashflow-export",
  "schema_version": 20261020000001,
  "app_version": "0.1.0",
  "exported_at": "2026-10-20T08:15:00.123456Z",
  "tables": {
    "account": [ { "id": "0199…", "name": "Compte courant", "kind": "CHECKING", "opening_balance": 185000, … } ],
    "ledger_transaction": [ … ],
    …
  }
}
```

- `schema_version` : version de la dernière migration appliquée. Une restauration exige la même version
  que l'application.
- `tables` : une entrée par table du modèle de données, une ligne par objet, colonnes telles qu'en base
  (cf. `modele-de-donnees.md` § « Correspondance physique SQLite ») : montants en unités mineures, taux
  en millionièmes, dates `YYYY-MM-DD`, horodatages ISO 8601 UTC, booléens `0`/`1`, champs JSON sous
  forme de chaîne. Les colonnes générées (`balance_date`) ne sont pas exportées.
- Ne sont **pas** exportés : l'utilisateur et ses sessions. Après une restauration, on définit le mot de
  passe avec `cashflow set-password`.
- Les tables `document` et `document_link` ne portent que la **description** des documents (titre, type,
  dates, note, nom de fichier, taille, empreinte SHA-256) et leurs rattachements. Leur contenu est dans le
  dossier `documents/` du volume, nommé par l'identifiant du document (ADR-0024) : pour une restauration
  intégrale, on copie ce dossier à côté de la base restaurée. L'archive ZIP de l'option « inclure les
  fichiers » (ADR-0008) les contiendra.
- L'index de recherche des documents (`document_search`) n'est pas exporté : il est dérivé des documents et se
  reconstruit après une restauration (`cashflow documents reindex`, R-DOC-19).

## Garanties

- Un export suivi d'une restauration puis d'un nouvel export donne des tables identiques (test
  `services::export::tests::export_roundtrip`).
- L'export est lu dans une seule transaction : il est cohérent même pendant l'utilisation.
