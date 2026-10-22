# Spec 07 — Projections

Statut : **Validée** (2026-10-20)
Modèle : [ProjectionScenario](../modele-de-donnees.md#projections--spec-07), RecurringRule, FixedSavingsTerms

## Objectif

Répondre à trois questions :
1. **Trésorerie** : mon compte courant sera-t-il à découvert avant la fin du mois ou dans 3 mois ?
2. **Épargne à taux déterminé** : combien vaudra mon Livret A dans 5 ans si je continue mes versements ?
3. **Épargne variable et patrimoine** : combien aurai-je dans 20 ans, selon un scénario prudent, médian ou optimiste ?

## Principes

- **R-PRJ-1** — Une projection est un **calcul pur** : elle ne crée, ne modifie ni ne stocke aucune
  transaction. Elle part de l'état réel à la date du jour (soldes calculés, dernières valorisations).
- **R-PRJ-2** — Le pas de temps est **mensuel**, sauf pour la trésorerie (pas journalier, cf. R-PRJ-10).
  Les valeurs projetées sont arrondies au centime à l'affichage seulement. Le calcul intermédiaire
  se fait en décimal exact (ADR-0001).
- **R-PRJ-3** — Toute projection affiche ses hypothèses (taux, versements, frais, horizon) et la date
  des données de départ. Une valorisation de départ « estimée » (R-CPT-6) est signalée.
- **R-PRJ-4** — Les **versements programmés** proviennent par défaut des récurrences de type virement
  (R-TRX-44) vers le compte concerné. Le scénario peut les remplacer ou les compléter.

## Projection de trésorerie (comptes `CASH`)

- **R-PRJ-10** — Solde projeté jour par jour = solde calculé aujourd'hui + transactions `PENDING`
  datées dans le futur + échéances prévues des récurrences (R-TRX-40) non encore honorées + échéances
  futures des prêts dont c'est le compte payeur (R-CPT-16), non encore payées.
- **R-PRJ-11** — Une dépense variable moyenne est **incluse par défaut**, et désactivable : moyenne
  des dépenses **non récurrentes** des 3 derniers mois complets, répartie uniformément sur les jours.
  Elle est affichée séparément des échéances connues.
- **R-PRJ-12** — Le solde minimum projeté et sa date sont mis en évidence ; un passage sous 0 (ou sous
  un seuil réglable par compte) déclenche un avertissement.

## Projection d'épargne à taux déterminé

- **R-PRJ-20** — On réutilise le **même moteur d'intérêts** que pour le calcul réel (spec 06), appliqué
  aux versements futurs. Il n'existe pas de second moteur simplifié.
- **R-PRJ-21** — Taux futur : par défaut, le dernier taux connu du barème reste constant. Le scénario
  peut fixer une trajectoire (« 2,4 % puis 2 % à partir de 2028 »).
- **R-PRJ-22** — Plafond : les versements programmés s'arrêtent quand le capital versé atteint
  `deposit_cap`. Le surplus est ignoré (V1), ou redirigé vers un compte de débordement choisi dans le
  scénario (V2).

## Projection d'épargne variable

- **R-PRJ-30** — Modèle déterministe (V1) : chaque mois,
  `V(m+1) = V(m) × (1 + r_m) × (1 − f_m) + versement(m)`, avec `r_m = (1 + r_annuel)^(1/12) − 1` et
  `f_m = 1 − (1 − frais_annuels)^(1/12)`. Les versements sont faits en fin de mois.
- **R-PRJ-31** — Un scénario fixe `r_annuel` et `frais_annuels` **par compte**, à défaut **par nature**
  de compte, à défaut par famille. Trois scénarios sont fournis par défaut, modifiables :

  | Scénario | Assurance vie (fonds euros dominant) | PEA / compte-titres actions | PER |
  |---|---|---|---|
  | Prudent | 1,5 % | 2 % | 2 % |
  | Médian | 2,5 % | 5 % | 4 % |
  | Optimiste | 3,5 % | 7 % | 6 % |

  Ce sont des **hypothèses par défaut, non des prévisions**. L'interface le dit explicitement.
- **R-PRJ-32** — Monte-Carlo (V2) : rendements mensuels tirés selon une loi log-normale paramétrée par
  un rendement espéré et une volatilité annuels, 10 000 tirages avec graine fixe, restitution des
  percentiles 10 / 50 / 90.
- **R-PRJ-33** — Inflation : si le scénario définit `inflation_rate`, les séries peuvent être affichées
  en euros constants (valeur / (1 + i)^(t/12)).

## Projection de patrimoine

- **R-PRJ-40** — Patrimoine projeté = Σ projections des comptes inclus dans le patrimoine, sous un même
  scénario. Les comptes `CASH` sont projetés à leur solde actuel, constant au-delà de l'horizon de
  trésorerie (pas de dérive spéculative). Un compte `LIABILITY` avec des conditions de prêt suit son
  tableau d'amortissement (R-CPT-16) : sa valeur actuelle diminuée des parts de capital des échéances
  futures. Sans conditions, il est projeté à sa dernière valeur, constante. Les comptes `PROPERTY` sont
  projetés à leur dernière valeur, sans plus-value ni dépréciation supposées (ADR-0019).
- **R-PRJ-41** — Les virements internes programmés sont pris en compte **des deux côtés** : ils
  réduisent le compte source et augmentent la cible. Le patrimoine n'est donc pas gonflé par les flux
  internes.

## Features

| ID | Priorité | Feature |
|---|---|---|
| PRJ-01 | V1 | Projection de trésorerie d'un compte courant (90 jours par défaut) |
| PRJ-02 | V1 | Projection d'un compte d'épargne à taux déterminé (horizon au choix) |
| PRJ-03 | V1 | Scénarios : prudent / médian / optimiste par défaut, édition des hypothèses |
| PRJ-04 | V1 | Projection d'épargne variable déterministe |
| PRJ-05 | V1 | Projection du patrimoine global, comparaison de scénarios sur un même graphique |
| PRJ-06 | V2 | Monte-Carlo avec bandes de percentiles |
| PRJ-07 | V2 | Objectifs : « X € à telle date » → versement mensuel nécessaire selon un scénario |
| PRJ-08 | V2 | Compte de débordement en cas de plafond atteint |
| PRJ-09 | V2 | Affichage en euros constants |

### PRJ-04 — Critère d'acceptation

- Étant donné V0 = 10 000 €, r = 5 %/an, frais = 0, versement = 0, alors la valeur à 12 mois vaut
  10 500,00 € (à l'arrondi près) et à 120 mois 16 288,95 €.
