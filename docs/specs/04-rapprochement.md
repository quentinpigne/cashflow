# Spec 04 — Rapprochement et points de contrôle

Statut : **Validée** (2026-10-20)
Modèle : [BalanceCheckpoint](../modele-de-donnees.md#rapprochement--spec-04)
Décisions : ADR-0002 (solde calculé et points de contrôle)

## Objectif

Garantir que le solde calculé de chaque compte correspond **au centime près** au solde annoncé
par la banque, et figer les périodes vérifiées.

## Concepts

- Un **point de contrôle** est une affirmation : « au soir du D, le compte A avait un solde de X ».
  Il vient d'un relevé (import) ou d'une saisie (je lis le solde sur l'appli de ma banque).
- L'**écart** d'un point de contrôle vaut `X − solde calculé(A, D)`.
- **Rapprocher** consiste à constater un écart nul et à le valider. Toutes les transactions de date
  ≤ D deviennent alors `RECONCILED` et sont verrouillées.

```
 opening ─────●────────────●──────────────●─────────► temps
          CP 31/01      CP 28/02        CP 31/03
          ✔ rapproché   ✔ rapproché     ⚠ écart −12,50 €
          (verrouillé)  (verrouillé)    → transactions du 01/03 au 31/03 à vérifier
```

## Règles

- **R-RAP-1** — Un point de contrôle exprime un solde **en fin de journée** D : il inclut toutes les
  transactions de **date de solde** ≤ D (`balance_date`, R-TRX-7), quel que soit leur statut de pointage.
- **R-RAP-2** — Un relevé importé avec un solde de fin crée un point de contrôle à `period_end` ; avec
  un solde de début, un point de contrôle à `period_start − 1 jour`. Un point de contrôle existant à
  la même date est remplacé après confirmation s'il diffère.
- **R-RAP-3** — L'écart de chaque point de contrôle est recalculé à chaque modification des
  transactions du compte ; il n'est jamais stocké.
- **R-RAP-4** — On peut valider (rapprocher) **tout** point de contrôle à écart nul, même si des points
  de contrôle antérieurs ne sont pas rapprochés. Comme le rapprochement au jour D verrouille toutes les
  transactions de date de solde ≤ D (R-RAP-5), les points de contrôle antérieurs non rapprochés sont
  traités à la validation :
  - ceux dont l'écart est nul sont rapprochés en même temps ;
  - ceux qui présentent un écart sont listés **avant** la validation (« cet écart sera figé ») ; après
    validation, ils restent non rapprochés et marqués « écart figé ». C'est typiquement le signe d'une
    transaction mal datée entre les deux dates, sans effet sur le solde au jour D.
- **R-RAP-5** — Valider un rapprochement passe en `RECONCILED` toutes les transactions du compte de
  date de solde ≤ D, y compris les `PENDING` restantes : la banque a confirmé le solde, ce qui vaut pointage.
  L'interface signale ces `PENDING` avant validation, car elles sont souvent suspectes.
- **R-RAP-6** — Déverrouiller un point de contrôle rapproché le repasse à l'état non rapproché, ainsi
  que tous les points de contrôle **postérieurs**, et repasse en `CLEARED` les transactions qui ne
  sont plus couvertes. Cette action demande une confirmation explicite.
- **R-RAP-7** — Toute écriture qui modifierait le solde à une date déjà couverte par un point de
  contrôle rapproché est refusée : création, suppression, changement de montant, de date de solde ou de compte
  (cf. R-TRX-3).
- **R-RAP-8** — Le solde d'ouverture d'un compte joue le rôle d'un point de contrôle implicite,
  toujours rapproché, à `opening_date − 1 jour`. Le modifier exige qu'aucun point de contrôle ne soit
  rapproché.
- **R-RAP-9** — Aide à la recherche d'écart. Quand un écart est non nul, l'application propose, entre
  le dernier point rapproché et le point en écart :
  - les transactions `PENDING` (souvent des saisies jamais passées en banque) ;
  - une transaction ou une combinaison de 2 transactions dont le montant égale l'écart (ou son opposé) ;
  - une transaction dont le montant égale **le double** de l'écart, ce qui trahit une erreur de signe ;
  - les jours où le solde calculé diverge des soldes intermédiaires du relevé, s'ils sont connus.
- **R-RAP-10** — Comptes `VARIABLE_SAVINGS` et `LIABILITY` : pas de point de contrôle. Le rôle équivalent est tenu
  par les valorisations (spec 06).
- **R-RAP-11** — Après la validation d'un import qui a créé ou mis à jour un point de contrôle à écart
  nul (R-IMP-16), l'écran de fin d'import **propose** « Rapprocher au <date> » en un clic. Rien n'est
  rapproché automatiquement.

## Features

| ID | Priorité | Feature |
|---|---|---|
| RAP-01 | MVP | Saisir un point de contrôle manuel et voir l'écart |
| RAP-02 | MVP | Points de contrôle créés par l'import (soldes de relevé) |
| RAP-03 | MVP | Valider un rapprochement (verrouillage) |
| RAP-04 | V1 | Déverrouiller un rapprochement |
| RAP-05 | V1 | Assistant de recherche d'écart (R-RAP-9) |
| RAP-06 | V1 | Vue « santé des comptes » : pour chaque compte, date du dernier rapprochement et écart courant |
| RAP-07 | V1 | Pointage manuel d'une transaction (`PENDING` → `CLEARED`) sans import |

### RAP-01 / RAP-03 — Rapprocher

- Étant donné un compte au solde d'ouverture de 1 000,00 € au 2026-01-01 et des transactions
  cumulées de −250,00 € jusqu'au 2026-01-31, quand je saisis un point de contrôle de 750,00 € au
  2026-01-31, alors l'écart vaut 0,00 € et je peux valider.
- Après validation, modifier le montant d'une transaction du 2026-01-15 est refusé avec un message
  qui indique le point de contrôle à déverrouiller.
- Quand je saisis 740,00 € au lieu de 750,00 €, alors l'écart affiché vaut −10,00 € et la validation
  est impossible.
