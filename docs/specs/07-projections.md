# Spec 07 — Projections

Statut : **Validée** (2026-10-20)
Modèle : [ProjectionScenario](../modele-de-donnees.md#projections--spec-07), RecurringRule, FixedSavingsTerms, Employment (spec 10)

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
  futures des prêts avec tableau dont c'est le compte payeur (R-PRE-3), non encore payées. Une récurrence du compte
  payeur dont le montant égale une échéance (à ±1 €) est alors ignorée : la mensualité ne compte qu'une
  fois. Les mensualités déjà ventilées (R-PRE-6) n'entrent pas dans la dépense variable moyenne
  (R-PRJ-11).
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
  scénario. Les comptes `CASH` partent de leur solde actuel, diminué des versements programmés (R-PRJ-41)
  et augmenté chaque mois de la capacité d'épargne (R-PRJ-42), sauf si le scénario la désactive
  (R-PRJ-47). Les biens et les prêts comptés dans le patrimoine s'y ajoutent (spec 09) : un prêt avec tableau suit son capital restant dû (R-PRE-4), un prêt en suivi simple garde son
  dernier capital restant dû, un bien sa dernière valeur (R-BIE-5).
- **R-PRJ-41** — Les virements internes programmés sont pris en compte **des deux côtés** : ils
  réduisent le compte source et augmentent la cible. Le patrimoine n'est donc pas gonflé par les flux
  internes.

## Capacité d'épargne (ADR-0022)

- **R-PRJ-42** — **Capacité d'épargne** du mois m = revenus projetés (R-PRJ-43) − dépenses projetées
  (R-PRJ-44) − échéances des prêts avec tableau datées en m (mensualité et assurance, R-PRE-3). Elle est
  calculée pour chaque mois de l'horizon à partir du mois suivant, et peut être négative.
- **R-PRJ-43** — **Revenus projetés** = salaires projetés des emplois (R-SAL-10, R-SAL-11) + **autres
  revenus moyens** : moyenne mensuelle, sur les 12 derniers mois complets, des ventilations de catégories
  `INCOME` des comptes `CASH`, hors catégories reliées à un emploi (et leurs sous-catégories). Les intérêts
  des comptes d'épargne n'y entrent pas : leur projection les compte déjà (R-PRJ-20). Les autres revenus
  restent constants. Sans emploi déclaré, les revenus projetés sont donc la moyenne des revenus passés.
- **R-PRJ-44** — **Dépenses projetées** = dépenses mensuelles moyennes (R-TDB-8), hors intérêts et
  assurance des mensualités ventilées (R-PRE-6), augmentées chaque 1er janvier du **taux de hausse des
  dépenses** du scénario (2 % par défaut dans les trois scénarios). Les remboursements des prêts en suivi
  simple et les acquisitions ne sont pas projetés (le prêt garde son capital restant dû, R-PRE-9). Un prêt
  avec tableau dont des échéances des 12 derniers mois ne sont pas payées est signalé : sa mensualité,
  comptée en dépense, le serait deux fois.
- **R-PRJ-45** — **Affectation**. La capacité d'épargne est créditée en fin de mois sur le **compte de
  versement** : le compte courant qui a reçu le plus de ventilations des catégories des emplois sur les 12
  derniers mois, à défaut le premier compte courant. Les versements programmés en partent comme avant
  (R-PRJ-4, R-PRJ-41). Le scénario peut désigner un **compte d'affectation du reste** (compte d'épargne) : le
  reste positif du mois (capacité − versements programmés des comptes `CASH`) y est alors viré, dans la
  limite de son plafond (R-PRJ-22) ; le dépassement reste sur le compte de versement.
- **R-PRJ-46** — La projection affiche la capacité d'épargne mois par mois avec ses composantes et ses
  hypothèses (taux de hausse des salaires et des dépenses, moyennes de départ, fins d'emploi et de prêts).
  Un solde projeté du compte de versement sous zéro déclenche un avertissement daté (« les versements
  programmés dépassent la capacité d'épargne à partir de … »).
- **R-PRJ-47** — La capacité d'épargne est **activée par défaut** dans chaque scénario, et désactivable :
  les comptes `CASH` gardent alors leur solde actuel, diminué des seuls versements programmés.

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
| PRJ-10 | V1 | Capacité d'épargne dans la projection du patrimoine (ADR-0022) |

### PRJ-04 — Critère d'acceptation

- Étant donné V0 = 10 000 €, r = 5 %/an, frais = 0, versement = 0, alors la valeur à 12 mois vaut
  10 500,00 € (à l'arrondi près) et à 120 mois 16 288,95 €.

### PRJ-10 — Critère d'acceptation

- Étant donné l'emploi de l'exemple de la spec 10 (scénario médian, 2 %), 50 € d'autres revenus moyens,
  1 900 € de dépenses moyennes, une hausse des dépenses de 2 %, un versement programmé de 300 € par mois du
  compte courant vers le Livret A et aucun prêt : la capacité d'épargne de novembre 2026 vaut
  2 700 + 50 − 1 900 = 850 € (compte courant +550 €, Livret A +300 €) ; celle de décembre 2026, 13e mois
  compris, 3 450 € ; celle de janvier 2027, 2 754 + 50 − 1 938 = 866 €.
- Avec en plus le prêt de PRE-02 (échéance de 1 159,20 €), dont 550 € d'intérêts et d'assurance comptés
  dans les 1 900 € : la capacité de novembre 2026 vaut 2 700 + 50 − 1 350 − 1 159,20 = 240,80 €, et elle
  augmente de 1 159,20 € (avant hausse des dépenses) le mois suivant la dernière échéance.
- Capacité d'épargne désactivée : le compte courant reste à son solde du jour, diminué des versements
  programmés.
