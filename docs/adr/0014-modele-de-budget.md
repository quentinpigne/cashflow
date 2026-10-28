# ADR-0014 — Modèle de budget : lignes par catégorie, projets et objectifs d'épargne

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : spec 05, spec 02, spec 08, modèle de données

## Contexte

Trois modèles avaient été envisagés (spec 05, version brouillon) : budget par catégorie, enveloppes
façon YNAB, ou hybride. Le point de décision du 2026-10-20 a fixé les besoins suivants :

| Question | Réponse |
|---|---|
| Usage | Piloter pendant le mois **et** analyser après coup |
| Granularité | Postes de budget = catégories d'analyse, un poste pouvant en regrouper plusieurs (« Voiture » : Carburant et Péage) |
| Budgets projet multi-catégories | Oui, important |
| Périodes | Mensuelle, annuelle, ponctuelle (projet) |
| Dépense de projet dans le budget mensuel | Au choix, par projet |
| Report du reliquat | Au choix, par ligne |
| Ligne annuelle | Suivi annuel **et** quote-part mensuelle |
| Comptes concernés | Au choix, par compte |
| Revenus | Prévu / réel, avec un « reste à budgéter » sans obligation d'arriver à zéro |
| Épargne | Objectif par compte cible |
| Reconduction | Modèle, plus des surcharges ponctuelles |
| Dépassement | Alertes visuelles et notifications |

## Options envisagées

1. **Budget par catégorie seul** — ❌ Ne sait pas isoler les dépenses d'un projet, puisqu'un restaurant
   en vacances a la même catégorie qu'un restaurant ordinaire.
2. **Enveloppes (YNAB)** — ❌ Double classement (catégorie + enveloppe) alors que postes et catégories
   sont identiques. ❌ La base zéro stricte n'est pas souhaitée.
3. **Lignes par catégorie + projets explicites + objectifs d'épargne** — ✅ Un seul classement au
   quotidien (la catégorie). Une ligne peut regrouper plusieurs catégories sans créer de second
   classement : elle se déduit de la catégorie. Le projet n'est qu'une affectation **optionnelle** de la
   ventilation.
   ✅ Le report par ligne apporte la souplesse des enveloppes là où on la veut.

## Décision

Option 3 :
- `BudgetLine` : une ou plusieurs catégories (avec un nom facultatif) ou un compte cible, nature
  `EXPENSE` / `INCOME` / `SAVINGS`, périodicité
  `MONTHLY` / `YEARLY`, montant, report optionnel, période de validité (versions).
- `BudgetOverride` : surcharge d'une ligne pour un mois ou une année budgétaire.
- `Project` : budget ponctuel ; la ventilation porte un `project_id` optionnel ; un drapeau par projet
  l'exclut ou non des lignes de période.
- `Account.include_in_budget` définit le périmètre.
- Rattachement d'une ventilation à une ligne : implicite, par la **ligne la plus spécifique** de sa
  catégorie (R-BUD-12). Une catégorie est dans au plus une ligne en vigueur (R-BUD-11).

## Conséquences

- `Split` gagne un champ `project_id`. C'est le seul ajout aux transactions.
- Les règles de catégorisation (spec 03) peuvent aussi rattacher à un projet (action `set_project`).
- Les notifications passent par le centre de notifications intégré (précisé par ADR-0015) ; le push
  sur téléphone est reporté en V2 (BUD-09).
- Les reports cumulés rendent sensibles les modifications rétroactives : elles sont permises, mais signalées (R-BUD-52).
