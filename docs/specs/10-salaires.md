# Spec 10 — Salaires

Statut : **Validée** (2026-10-22)
Modèle : [Employment, Salary, SalaryBonus](../modele-de-donnees.md#salaires--spec-10)
Décision : ADR-0022 (salaires déclarés et capacité d'épargne projetée)

## Concepts

Un **emploi** (employeur, poste, dates) porte l'historique de sa paie : des **rémunérations** successives,
chacune en vigueur à partir d'un mois d'effet (brut annuel de base, net mensuel versé), et des **primes**
datées par leur versement (13e mois, prime, intéressement…). Cet historique est **déclaré**, à partir du
contrat, des avenants et des fiches de paie. Il ne remplace pas les transactions : le salaire reçu reste
une transaction du compte qui le reçoit, catégorisée en revenu.

Chaque emploi est relié à une **catégorie de revenu**. Chaque mois, le net attendu est comparé au reçu sur
cette catégorie : un écart signale une rémunération oubliée ou une anomalie de paie.

Le salaire se **projette** : hausse annuelle selon le scénario, augmentations prévues, changement d'emploi,
fin d'activité. Il entre dans la **capacité d'épargne** de la projection du patrimoine (R-PRJ-42).

La rubrique est **facultative**, comme Biens et Prêts.

## Emplois

- **R-SAL-1** — Un **emploi** a un employeur, une **date de début**, une **catégorie de revenu** (« Salaires »
  par défaut) et un **mois des augmentations** (janvier par défaut) : le mois où tombent habituellement les
  hausses, qui sert à la projection (R-SAL-10). Optionnels : le poste, la date de fin, une note, et le
  **versement décalé** (le salaire du mois M est versé en M+1). Deux emplois peuvent se chevaucher. Une date
  de fin ou de début future décrit un changement d'emploi ou une fin d'activité prévus. Montants en euros.
- **R-SAL-2** — Une **rémunération** a un **mois d'effet**, un **brut annuel de base** (hors primes : un 13e
  mois se saisit comme prime), un **net mensuel versé** (après prélèvement à la source), tous deux positifs,
  et un **motif** : `HIRE` embauche, `RAISE` augmentation, `PROMOTION` promotion, `TAX_RATE` changement de
  taux de prélèvement à la source, `OTHER` autre. Note optionnelle. Une seule par mois d'effet, entre le mois
  de début et le mois de fin de l'emploi. Le formulaire préremplit les montants de la précédente.
- **R-SAL-3** — La rémunération **en vigueur** au mois M est la dernière dont le mois d'effet est au plus
  tard M. Un mois de l'emploi antérieur à toute rémunération est **non renseigné**.
- **R-SAL-4** — Une rémunération dont le mois d'effet est futur est **prévue** : elle s'affiche comme telle
  et la projection l'applique dans tous les scénarios. Elle devient une rémunération ordinaire quand son mois
  arrive, sans action.
- **R-SAL-5** — Une **prime** a un libellé, une nature (`THIRTEENTH_MONTH` 13e mois, `BONUS` prime,
  `PROFIT_SHARING` intéressement ou participation, `OTHER` autre), une **date de versement** pendant l'emploi,
  un net versé positif, un brut optionnel, et le drapeau **récurrente** (oui par défaut pour un 13e mois, non
  sinon). Une prime à date future est prévue (comme R-SAL-4). Un intéressement placé directement sur un plan
  d'épargne salariale n'est pas une prime : il se suit par les valorisations de ce compte (ADR-0010).
- **R-SAL-6** — Supprimer un emploi supprime ses rémunérations et ses primes. Les transactions ne changent pas.

## Évolution

- **R-SAL-7** — **Revenu annuel** d'un emploi pour une année Y :
  - net = Σ, sur les mois de Y couverts par l'emploi, du net en vigueur + Σ des nets des primes versées en Y ;
  - brut = Σ, sur les mêmes mois, du brut annuel en vigueur / 12 + Σ des bruts des primes versées en Y.
  L'année est **incomplète** si l'un de ses mois est non renseigné (R-SAL-3), son brut aussi si une de ses
  primes n'a pas de brut. L'année en cours est **prévisionnelle** : ses mois à venir comptent la rémunération
  en vigueur ou prévue, les primes prévues et les primes récurrentes (R-SAL-11), sans hausse automatique.
- **R-SAL-8** — L'évolution affiche, année par année, le net et le brut annuels de chaque emploi et de tous
  les emplois, et leur variation en pourcentage sur l'année précédente. Une variation n'est pas calculée
  quand l'une des deux années est incomplète ou ne couvre pas 12 mois d'emploi.

## Comparaison avec le reçu

- **R-SAL-9** — Pour une catégorie de revenu et un mois M :
  - **attendu** = Σ, sur les emplois reliés à cette catégorie, du net en vigueur au mois de paie (M, ou M−1
    en versement décalé) + Σ de leurs primes versées en M ;
  - **reçu** = Σ des ventilations de cette catégorie et de ses sous-catégories datées en M, tous comptes
    confondus ;
  - **écart** = reçu − attendu, **signalé** quand sa valeur absolue dépasse la tolérance (5 % de l'attendu par
    défaut, réglable).
  Ne sont jamais signalés : le mois en cours, un mois partiel (début ou fin d'emploi), un mois non renseigné.
  L'écart signalé du dernier mois complet apparaît dans les alertes de l'accueil.

## Projection

- **R-SAL-10** — **Net mensuel projeté** d'un emploi, chaque mois de l'horizon à partir du mois suivant,
  pendant l'emploi seulement (rien avant sa date de début ni après sa date de fin) :
  - il part du net en vigueur ce mois-là, rémunérations prévues comprises (R-SAL-4) ;
  - il augmente du **taux de hausse des salaires** du scénario à chaque mois des augmentations de l'emploi
    postérieur au mois en cours **et** au mois d'effet de sa dernière rémunération (passée ou prévue) : une
    augmentation prévue remplace la hausse automatique de l'année ;
  - le calcul est exact, arrondi au centime à l'affichage seulement (R-PRJ-2).
  Un emploi futur sans rémunération ne projette rien ; les hypothèses le signalent.
- **R-SAL-11** — Une prime **récurrente**, si c'est la dernière de son libellé pour l'emploi, se répète chaque
  année à la même date pendant l'emploi. Son net est celui de sa dernière occurrence, augmenté du taux de
  hausse des salaires autant de fois que le net mensuel a été augmenté automatiquement (R-SAL-10) depuis.
- **R-SAL-12** — Le taux de hausse des salaires est fixé par scénario : 1 % (prudent), 2 % (médian), 3 %
  (optimiste) par défaut. Ce sont des hypothèses, non des prévisions (comme R-PRJ-31).
- **R-SAL-13** — La projection affiche, année par année, le net annuel projeté de chaque scénario, dans la
  continuité de l'évolution réelle (R-SAL-8), avec ses hypothèses : taux, rémunérations et primes prévues,
  dates de fin d'emploi.

## Activation

- **R-SAL-14** — La rubrique **Salaires** s'active dans les réglages, comme Biens et Prêts (R-BIE-7) :
  désactivable seulement s'il n'existe aucun emploi ; désactivée, l'interface n'en montre rien et aucun écart
  n'est signalé. Une nouvelle installation l'a désactivée. La capacité d'épargne (R-PRJ-42) existe sans elle :
  les revenus projetés sont alors la moyenne des revenus passés.

## Features

| ID | Priorité | Feature |
|---|---|---|
| SAL-01 | V1 | Créer, modifier, supprimer un emploi ; ses rémunérations et ses primes, passées ou prévues |
| SAL-02 | V1 | Évolution annuelle du salaire, par emploi et tous emplois |
| SAL-03 | V1 | Comparaison mensuelle de l'attendu et du reçu, alerte sur l'accueil |
| SAL-04 | V1 | Projection du salaire par scénario (taux de hausse, rémunérations prévues, fin d'emploi) |
| SAL-05 | V1 | Activer ou désactiver la rubrique Salaires |
| SAL-06 | V2 | Pension de retraite et revenus de remplacement projetés (Q20) |
| SAL-07 | V2 | Salaire déclaré dans la trésorerie projetée du compte qui le reçoit |
| SAL-08 | V2 | Préremplir une rémunération à partir des salaires reçus |

Exemple commun : un emploi chez Acme depuis le 2022-09-01, catégorie « Salaires », mois des augmentations
janvier. Rémunérations : 2022-09 (embauche) 38 000 € brut, 2 400 € net ; 2024-01 (augmentation) 40 000 €,
2 550 € ; 2026-01 (augmentation) 42 500 €, 2 700 €. Prime « 13e mois » récurrente le 2025-12-19 : 2 600 € net,
3 400 € brut. Aujourd'hui : 2026-10-22.

### SAL-02 — Évolution

- En 2025, le net annuel vaut 12 × 2 550 + 2 600 = 33 200 € et le brut 40 000 + 3 400 = 43 400 €.
- En 2022, l'emploi couvre 4 mois : net 9 600 €, brut 12 666,67 € ; l'année n'est pas incomplète, mais sa
  variation n'est pas calculée.

### SAL-03 — Comparaison

- Septembre 2026 : 2 702,15 € reçus sur « Salaires » ; écart +2,15 €, non signalé.
- Si rien n'est reçu en septembre, l'écart de −2 700,00 € est signalé et apparaît dans les alertes de l'accueil.
- En versement décalé, le salaire de septembre est attendu en octobre.

### SAL-04 — Projection

- Scénario médian (2 %) : net de novembre 2026 2 700 €, de janvier 2027 2 754 €, de janvier 2028 2 809,08 € ;
  13e mois de décembre 2026 2 600 €, de décembre 2027 2 652 €. Net annuel projeté 2027 : 35 700 €.
- Avec une rémunération prévue en 2027-03 à 2 900 € net : pas de hausse automatique en janvier 2027 ; net
  2 700 € en janvier et février 2027, 2 900 € en mars, 2 958 € en janvier 2028.
- Avec une date de fin d'emploi au 2027-06-30 : aucun salaire à partir de juillet 2027, et pas de 13e mois en
  décembre 2027.

### SAL-05 — Activation

- Sur une nouvelle installation, « Salaires » n'apparaît pas dans le menu ; tant qu'un emploi existe,
  l'interrupteur ne peut pas être désactivé.
