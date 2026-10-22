# ADR-0022 — Salaires déclarés et capacité d'épargne projetée

- **Statut** : Accepté
- **Date** : 2026-10-22
- **Concerne** : vision, spec 07, spec 10, modèle de données, glossaire, écrans, feuille de route

## Contexte

Le salaire n'existe aujourd'hui que sous forme de transactions catégorisées « Salaires » et, au mieux,
d'une récurrence de revenu. Trois besoins ne sont pas couverts :

- **suivre l'évolution du salaire au fil des années** : brut, net, primes, employeurs successifs. Les
  transactions donnent le net reçu, pas le brut, et mélangent salaire de base, primes et régularisations ;
- **projeter le salaire** : hausses annuelles, augmentations prévues, changement d'emploi, fin d'activité ;
- **projeter le patrimoine avec les revenus** : la projection du patrimoine (R-PRJ-40) garde les comptes de
  liquidités à leur solde actuel, et l'épargne ne grossit que par les virements programmés. Ce qu'il reste
  chaque mois entre revenus et dépenses est ignoré, tout comme la fin d'un prêt ou une baisse de revenus.

## Options envisagées

Source de l'historique :

1. **Déduit des transactions** (catégorie « Salaires »). ✅ Aucune saisie. ❌ Ni brut, ni distinction entre
   salaire et primes, ni employeur ; une régularisation ou un versement décalé fausse l'historique.
2. **Déclaré** : rémunérations datées saisies à partir du contrat, des avenants et des fiches de paie.
   ✅ Brut, net, primes et employeur exacts. ❌ Saisie, peut diverger du reçu sans qu'on le voie.
3. **Déclaré et comparé au reçu.** ✅ Les avantages de 2, et l'écart avec les transactions signale un oubli
   de saisie ou une anomalie de paie. ❌ Il faut relier chaque emploi à une catégorie de revenu.

Intégration dans la projection du patrimoine :

1. **Courbe à part** : le salaire projeté s'affiche, sans effet sur le patrimoine. ❌ Ne répond pas au
   besoin.
2. **Versements indexés** : les virements programmés vers l'épargne suivent la hausse du salaire. ❌ Ne
   voit ni les dépenses, ni la fin d'un prêt, ni ce qui reste sur le compte courant.
3. **Capacité d'épargne** : chaque mois, revenus projetés − dépenses projetées − échéances de prêts =
   capacité d'épargne. Elle finance les versements programmés, et le reste s'accumule sur un compte choisi
   dans le scénario. ✅ Le patrimoine projeté reflète salaire, dépenses, prêts et versements. ❌ Dépend
   d'hypothèses supplémentaires (hausse des salaires, hausse des dépenses), qu'il faut afficher.

## Décision

Source **déclarée et comparée** (option 3). Un **emploi** (employeur, poste, dates) porte des
**rémunérations** successives datées par mois d'effet (brut annuel de base, net mensuel versé) et des
**primes** datées par leur versement (13e mois, prime, intéressement…). Chaque emploi est relié à une
catégorie de revenu : chaque mois, le net attendu est comparé au reçu de cette catégorie, et l'écart est
signalé au-delà d'une tolérance.

Le net retenu est le **net versé**, après prélèvement à la source : c'est ce qui arrive sur le compte.
L'application ne calcule ni cotisations ni impôt (cf. non-objectif « déclaration fiscale »).

Projection du salaire : le dernier net, ou la rémunération prévue (une rémunération dont le mois d'effet est
futur), augmente chaque année, au mois des augmentations de l'emploi, d'un **taux de hausse des salaires**
fixé par scénario. Un changement d'emploi prévu se décrit par la fin de l'emploi en cours et un nouvel
emploi daté dans le futur ; une fin d'activité par la seule date de fin. Les primes récurrentes sont
projetées chaque année.

Intégration par la **capacité d'épargne** (option 3) : la projection du patrimoine crédite chaque mois la
capacité d'épargne sur les comptes de liquidités. Les revenus qui ne viennent pas d'un emploi déclaré et les
dépenses sont pris en moyenne sur 12 mois ; les dépenses augmentent d'un **taux de hausse des dépenses**
fixé par scénario. Les échéances des prêts avec tableau sont comptées exactement, ce qui fait apparaître
leur fin. Sans emploi déclaré, les revenus projetés sont la moyenne des revenus passés : le modèle reste
valable, salaire constant.

## Conséquences

- La projection du patrimoine change : les comptes de liquidités ne sont plus constants (R-PRJ-40). Un
  scénario peut désactiver la capacité d'épargne et revenir à l'ancien comportement.
- Deux hypothèses de plus par scénario (hausse des salaires, hausse des dépenses), affichées avec les
  autres (R-PRJ-3).
- Le domaine gagne une projection de salaire et un calcul de capacité d'épargne, purs et testés.
- Nouvelle rubrique activable **Salaires**, désactivée par défaut, comme Biens et Prêts.
- Hors périmètre de la V1 : pension de retraite et autres revenus de remplacement (Q20), calcul du net à
  partir du brut, salaire déclaré dans la trésorerie projetée.
