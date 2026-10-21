# Spec 05 — Budgets

Statut : **Validée** (2026-10-20)
Modèle : [Project, BudgetLine, BudgetOverride](../modele-de-donnees.md#budgets--spec-05)
Décision : ADR-0014 (modèle de budget), issue du point dédié du 2026-10-20

## Concepts

Le budget combine trois mécanismes, qui reposent tous sur les ventilations (ADR-0003) :

| Mécanisme | Ce qu'il suit | Rattachement |
|---|---|---|
| **Ligne de budget** (`BudgetLine`) | Une catégorie de dépense ou de revenu, par **mois** ou par **an** | Implicite, par la catégorie de la ventilation |
| **Objectif d'épargne** (`BudgetLine` de nature `SAVINGS`) | Les virements vers **un compte d'épargne** donné, par mois | Implicite, par les virements internes vers ce compte |
| **Projet** (`Project`) | Un budget ponctuel, multi-catégories et multi-mois (vacances, travaux) | **Explicite** : la ventilation porte `project_id` |

```
Transaction −120 €  « RISTORANTE DA MARIO »
 └─ Split −120 €  catégorie Restaurants · projet « Italie 2027 »
       │                    │
       │                    └─► Projet « Italie 2027 » : 1 830 / 2 500 €
       └─► Ligne mensuelle « Restaurants » : seulement si le projet n'exclut pas ses dépenses (R-BUD-21)
```

Le budget est un **modèle unique, versionné dans le temps** : les lignes ont une période de validité,
et un mois précis peut être **surchargé** (décembre : Cadeaux 300 € au lieu de 50 €).

### Vue mensuelle (exemple)

```
Mars 2027                                  Prévu      Réel     Reste
─────────────────────────────────────────────────────────────────────
REVENUS
  Salaires                               3 200,00  3 200,00
  Autres revenus                             0,00     45,00
DÉPENSES MENSUELLES
  Supermarché            (report +32,10)   600,00    540,20     91,90 ▓▓▓▓▓▓▓▓░
  Restaurants                              150,00    162,00    −12,00 ▓▓▓▓▓▓▓▓▓▓ ⚠
  Carburant                                120,00     64,50     55,50
DÉPENSES ANNUELLES (quote-part du mois)
  Assurance auto        900 €/an → 75,00/mois   consommé sur l'année : 0 / 900
ÉPARGNE
  → Livret A                               200,00    200,00      0,00 ✔
  → Assurance vie                          400,00      0,00    400,00
HORS BUDGET (dépenses sans ligne)                    37,80
NON CATÉGORISÉ                                       12,00
─────────────────────────────────────────────────────────────────────
Reste à budgéter (prévu) : 3 200 − 870 − 75 − 600 = 1 655,00 €
Solde du mois (réel)     : 3 245 − 816,50 − 200  = 2 228,50 €
```

## Règles

### Périmètre

- **R-BUD-1** — Chaque compte porte un drapeau `include_in_budget`. Valeurs par défaut : `true` pour
  la famille `CASH`, `false` pour `FIXED_SAVINGS`. Il est toujours `false` pour `VARIABLE_SAVINGS`,
  dont les transactions ne sont que des apports (R-EPA-20), et pour `LIABILITY` (R-CPT-12).
- **R-BUD-2** — Seules les ventilations des transactions de comptes `include_in_budget` alimentent les
  lignes de dépense et de revenu. Les jambes de virement interne n'alimentent jamais ces lignes
  (R-TRX-32), seulement les objectifs d'épargne (R-BUD-30).
- **R-BUD-3** — Une ventilation est rattachée à une période budgétaire par la `date` de sa transaction
  (et non sa `bank_date`, R-TRX-7).

### Lignes de dépense et de revenu

- **R-BUD-10** — Une ligne de budget porte sur **une** catégorie, avec une nature (`EXPENSE` ou `INCOME`,
  égale à celle de la catégorie), une périodicité (`MONTHLY` ou `YEARLY`) et un montant prévu positif.
- **R-BUD-11** — À une date donnée, une catégorie a **au plus une** ligne en vigueur, quelle que soit sa
  périodicité.
- **R-BUD-12** — **Ligne la plus spécifique** : une ventilation alimente la ligne de sa propre catégorie
  si elle existe, sinon celle de sa catégorie parente si elle existe, sinon elle est « hors budget ».
  Une ligne sur « Alimentation » capte donc les sous-catégories qui n'ont pas leur propre ligne.
- **R-BUD-13** — Réel d'une ligne de dépense = −Σ des ventilations rattachées sur la période. Les
  remboursements (ventilations positives) viennent en déduction (R-TRX-13). Réel d'une ligne de
  revenu = +Σ des ventilations.
- **R-BUD-14** — Les ventilations sans catégorie apparaissent dans un groupe « non catégorisé » de la vue
  budget, séparé du « hors budget ».

### Ligne annuelle

- **R-BUD-15** — Une ligne `YEARLY` est suivie sur une **année budgétaire** commençant au mois
  `year_start_month` (1 = janvier par défaut ; 3 pour une assurance renouvelée en mars). Son réel est le
  cumul des ventilations rattachées depuis le début de l'année budgétaire.
- **R-BUD-16** — **Quote-part mensuelle** : dans la vue d'un mois, une ligne annuelle affiche
  `montant / 12`. La répartition en centimes est exacte : 11 quotes-parts arrondies à l'inférieur, et
  la dernière absorbe le reste (1 000 € → 11 × 83,33 + 83,37). La quote-part entre dans le « reste à
  budgéter » du mois (R-BUD-40). Le réel de la ligne n'est pas ventilé par mois : il est affiché en
  cumul annuel.

### Report du reliquat

- **R-BUD-17** — Chaque ligne de dépense a une option `rollover`. Désactivée, chaque période repart de
  zéro. Activée, le disponible de la période P vaut
  `prévu(P) + Σ (prévu(p) − réel(p))` pour les périodes p de la version de ligne en vigueur antérieures
  à P. Le report peut être négatif : un dépassement réduit le disponible suivant.
- **R-BUD-18** — Le cumul repart de zéro à chaque nouvelle **version** de la ligne (R-BUD-51), ou par une
  action explicite « remettre le report à zéro », qui crée une nouvelle version au même montant.
- **R-BUD-19** — Les lignes de revenu n'ont pas de report.

### Projets

- **R-BUD-20** — Un projet a un nom, une date de début, une date de fin optionnelle, un montant prévu
  global et un statut (`ACTIVE`, `CLOSED`). Une ventilation porte **au plus un** projet.
- **R-BUD-21** — Le drapeau `exclude_from_period_budgets` du projet (par défaut `true`) détermine si ses
  ventilations sont **retirées** des lignes mensuelles et annuelles. Dans les deux cas, elles restent
  comptées dans les analyses par catégorie (spec 08).
- **R-BUD-22** — Réel d'un projet = −Σ des ventilations rattachées, toutes catégories confondues,
  remboursements déduits. La vue projet ventile ce réel par catégorie.
- **R-BUD-23** — Une ventilation peut être rattachée à un projet quelle que soit sa date : un billet
  d'avion acheté 4 mois avant le départ est permis. Une date hors de [début, fin] est seulement signalée.
- **R-BUD-24** — On ne peut rattacher de nouvelles ventilations qu'à un projet `ACTIVE`. Clôturer un
  projet fige son bilan, sans rien verrouiller : les ventilations restent modifiables.
- **R-BUD-25** — Les ventilations des comptes hors budget (R-BUD-1) peuvent aussi être rattachées à un
  projet : un projet suit une dépense, quel que soit le compte qui l'a payée.

### Objectifs d'épargne

- **R-BUD-30** — Un objectif d'épargne est une ligne de nature `SAVINGS`, mensuelle, qui vise un compte
  (`target_account_id`) de famille `FIXED_SAVINGS` ou `VARIABLE_SAVINGS`.
- **R-BUD-31** — Réel d'un objectif sur un mois = Σ des virements internes entrant sur le compte cible
  depuis un compte `include_in_budget` − Σ des virements internes sortant du compte cible vers un compte
  `include_in_budget`, sur le mois. Un retrait de l'épargne vers le compte courant réduit donc le réel.
- **R-BUD-32** — Un objectif d'épargne peut avoir un report (R-BUD-17), pour rattraper un mois sans versement.
- **R-BUD-33** — Un versement sur le compte cible depuis un compte **non suivi** n'alimente pas
  l'objectif : ce n'est pas un virement interne.

### Synthèse du mois

- **R-BUD-40** — **Reste à budgéter** (prévu) = revenus prévus − Σ dépenses mensuelles prévues
  − Σ quotes-parts annuelles − Σ objectifs d'épargne. Il peut être positif ou négatif ; aucune
  obligation d'arriver à zéro.
- **R-BUD-41** — **Solde du mois** (réel) = revenus réels − dépenses réelles (lignes, hors budget et
  non catégorisées) − épargne réelle des objectifs. Les dépenses de projets exclus (R-BUD-21) y sont
  incluses, sur une ligne « projets » distincte.

### Modèle et surcharges

- **R-BUD-50** — Le budget est un ensemble de **versions de lignes**, chacune valable de
  `effective_from` (un mois) à `effective_to` (un mois, optionnel). Le budget d'un mois M est
  l'ensemble des versions en vigueur en M, auxquelles on applique les surcharges de M.
- **R-BUD-51** — Modifier le montant d'une ligne propose deux portées :
  - **« À partir de ce mois »** : la version courante est close au mois M−1 et une nouvelle version
    démarre en M. Les mois passés ne changent pas.
  - **« Ce mois seulement »** : une surcharge (`BudgetOverride`) est créée pour M (ou pour l'année
    budgétaire dans le cas d'une ligne annuelle).
- **R-BUD-52** — Les mois passés restent modifiables (surcharge ou version), avec avertissement : cela
  change les reports cumulés.
- **R-BUD-53** — Supprimer une ligne revient à la clore au mois précédent. Son historique reste visible.

### Alertes

- **R-BUD-60** — Chaque ligne de dépense, chaque projet et chaque objectif d'épargne a des seuils
  d'alerte (par défaut 80 % et 100 % du disponible, réglables globalement et par ligne). Une ligne ou un
  projet peut aussi **n'avoir aucune alerte** (budget fixe comme un loyer ou une échéance de prêt) :
  ni jauge orange ou rouge, ni notification, ni objectif signalé comme manqué. Un seuil est franchi
  quand le réel le **dépasse** (« au-delà », R-BUD-61).
- **R-BUD-61** — Affichage : jauge neutre sous le premier seuil, orange entre les deux, rouge au-delà
  du second. Pour un objectif d'épargne, la logique est inversée : on signale l'objectif **non atteint**
  en fin de mois.
- **R-BUD-62** — Notifications : au **premier** franchissement d'un seuil sur une période, une
  notification est émise, une seule fois par ligne, par seuil et par période (`dedupe_key`). Si le réel
  repasse sous le seuil puis le refranchit, rien n'est réémis. Le canal est le **centre de notifications
  de l'application** (ADR-0015) ; un badge signale les notifications non lues.
- **R-BUD-63** — Le franchissement est évalué après chaque écriture qui modifie des ventilations, y
  compris la validation d'un import. Un import qui fait franchir plusieurs seuils produit une
  notification groupée.

## Features

| ID | Priorité | Feature |
|---|---|---|
| BUD-01 | V1 | Drapeau `include_in_budget` par compte |
| BUD-02 | V1 | Lignes mensuelles de dépense et de revenu : création, versions, surcharges |
| BUD-03 | V1 | Vue mensuelle : prévu / réel / reste, hors budget, non catégorisé, reste à budgéter, solde du mois |
| BUD-04 | V1 | Report du reliquat par ligne |
| BUD-05 | V1 | Lignes annuelles avec quote-part mensuelle et suivi cumulé |
| BUD-06 | V1 | Projets : création, rattachement des ventilations (y compris en masse et par règle), vue projet |
| BUD-07 | V1 | Objectifs d'épargne par compte cible |
| BUD-08 | V1 | Alertes visuelles et centre de notifications intégré |
| BUD-09 | V2 | Notifications push sur le téléphone (ntfy ou Web Push), via un second canal de notification |
| BUD-10 | V1 | Assistant de création initiale : propose des montants à partir de la moyenne des 3 à 6 derniers mois par catégorie |
| BUD-11 | V2 | Ventilation prévue d'un projet par catégorie (prévu par catégorie au sein du projet) |
| BUD-12 | V2 | Alerte prévisionnelle : le rythme de dépense projette un dépassement en fin de période |
| BUD-13 | V2 | Historique : prévu vs réel sur 12 mois par ligne |

### Critères d'acceptation

**BUD-03 / R-BUD-12 — ligne la plus spécifique**
- Étant donné une ligne mensuelle « Alimentation » de 800 € et une ligne « Restaurants » (sous-catégorie
  d'Alimentation) de 150 €, quand je dépense 60 € en « Supermarché » et 40 € en « Restaurants », alors la
  ligne Alimentation affiche 60 € de réel et la ligne Restaurants 40 €.

**BUD-04 / R-BUD-17 — report**
- Étant donné une ligne « Supermarché » de 600 € avec report depuis janvier, un réel de 550 € en janvier
  et de 640 € en février, alors le disponible de mars vaut 600 + 50 − 40 = 610 €.

**BUD-05 / R-BUD-16 — annuel**
- Étant donné une ligne annuelle « Assurance auto » de 900 € (année civile), alors la vue de chaque mois
  affiche une quote-part de 75,00 €, et le paiement de 870 € en mars affiche « 870 / 900 € » sur l'année.

**BUD-06 / R-BUD-21 — projet exclu**
- Étant donné le projet « Italie 2027 » avec `exclude_from_period_budgets = true`, quand une ventilation
  « Restaurants » de 80 € est rattachée au projet, alors la ligne mensuelle Restaurants n'augmente pas,
  le projet augmente de 80 €, et l'analyse par catégorie (TDB-02) compte bien 80 € de restaurants.

**BUD-07 / R-BUD-31 — objectif d'épargne**
- Étant donné un objectif de 200 €/mois vers le Livret A, un virement de 300 € du compte courant vers le
  livret le 5 et un retrait de 100 € du livret vers le compte courant le 20, alors le réel du mois vaut 200 €.

**BUD-08 / R-BUD-62 — notification unique**
- Étant donné une ligne Restaurants de 150 € à 110 € de réel, quand j'importe un relevé qui la porte à
  165 €, alors une seule notification groupée signale le franchissement de 80 % et de 100 %.
