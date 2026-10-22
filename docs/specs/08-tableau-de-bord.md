# Spec 08 — Tableau de bord et indicateurs

Statut : **Validée** (2026-10-20)

## Principe

Chaque indicateur a **une définition écrite ici**, testée, et accessible dans l'interface (info-bulle
« comment est-ce calculé ? »). Aucun chiffre de démonstration : sans donnée, un indicateur affiche
« — » et explique ce qui manque.

## Définitions

Sauf mention contraire, les indicateurs portent sur un **mois civil** M et sur les comptes en EUR
(R-CPT-2). Les virements internes sont exclus des revenus et des dépenses (R-TRX-32). Les flux sont
rattachés au mois par leur `date` ; les soldes et le patrimoine utilisent la date de solde (R-TRX-7).

| Indicateur | Définition | Règle |
|---|---|---|
| **Patrimoine** (net) | Σ des soldes calculés (comptes `CASH` et `FIXED_SAVINGS`) et des valeurs (comptes `VARIABLE_SAVINGS`, R-CPT-6) des comptes `include_in_net_worth`, + Σ des valeurs des biens (R-BIE-2) − Σ des capitaux restants dus des prêts (R-PRE-4) comptés dans le patrimoine, à la date D. Le détail affiche l'actif brut (comptes et biens) et les dettes (prêts) séparément. | R-TDB-1 |
| **Variation de patrimoine** | Patrimoine(fin M) − patrimoine(fin M−1), décomposée en **épargne du mois** (flux) et **effet de marché + intérêts** (le reste) | R-TDB-2 |
| **Revenus** | Σ des ventilations sur des catégories `INCOME` | R-TDB-3 |
| **Dépenses** | −Σ des ventilations sur des catégories `EXPENSE` (les remboursements viennent en déduction) | R-TDB-4 |
| **Non catégorisé** | Σ des ventilations sans catégorie, hors virements, remboursements et acquisitions (R-TRX-37), séparément en entrées et en sorties | R-TDB-5 |
| **Épargne du mois** | Revenus − dépenses | R-TDB-6 |
| **Taux d'épargne** | Épargne du mois / revenus. Non défini si les revenus valent 0. Moyenne glissante sur 12 mois affichée à côté. | R-TDB-7 |
| **Dépenses mensuelles moyennes** | Moyenne des dépenses des N derniers mois **complets** (N = 12 par défaut, réglable) | R-TDB-8 |
| **Épargne de précaution (mois)** | Σ des soldes des comptes marqués `include_in_emergency_fund` (R-CPT-13) / dépenses mensuelles moyennes | R-TDB-9 |
| **Santé des comptes** | Par compte : date du dernier rapprochement, écart courant, nombre de transactions `PENDING` de plus de 10 jours | R-TDB-10 |
| **Intérêts de l'année** | Σ des intérêts courus (spec 06) des comptes `FIXED_SAVINGS` depuis le 1er janvier | R-TDB-11 |
| **Performance épargne variable** | TRI depuis le 1er janvier et depuis l'ouverture, par compte et global (spec 06) | R-TDB-12 |

- **R-TDB-13** — Un indicateur calculé sur un mois en cours est marqué « mois en cours ». Les
  comparaisons se font avec le mois précédent **à la même date** (M−1 du 1er au jour J), pas avec le
  mois précédent complet.

## Écrans

| ID | Priorité | Contenu |
|---|---|---|
| TDB-01 | MVP | **Accueil** : patrimoine et variation, liste des comptes groupés par famille avec leur solde, revenus / dépenses / épargne du mois, transactions à catégoriser, alertes (écarts de rapprochement, `PENDING` anciennes) |
| TDB-02 | V1 | **Dépenses par catégorie** : mois choisi, comparaison M−1 et moyenne sur 12 mois, drill-down vers les transactions |
| TDB-03 | V1 | **Évolution** : revenus, dépenses et épargne sur 12 / 24 mois ; taux d'épargne |
| TDB-04 | V1 | **Patrimoine** : historique par famille de comptes, biens et prêts, projection (spec 07) dans la continuité de la courbe |
| TDB-05 | V1 | **Épargne** : intérêts de l'année, performance des comptes variables, épargne de précaution |
| TDB-06 | V1 | **Budgets sur l'accueil** : jauges des lignes en alerte, reste à budgéter, objectifs d'épargne du mois, projets actifs (cf. spec 05, BUD-03) |
| TDB-08 | V1 | **Biens et prêts sur l'accueil** : un encadré Biens (valeur de chaque bien) et un encadré Prêts (capital restant dû, prochaine échéance), masqués si la rubrique est désactivée (spec 09) |
| TDB-07 | V1 | **Filtre projets** dans l'analyse par catégorie (TDB-02) : inclure ou exclure les dépenses rattachées à des projets |

### TDB-01 — Critères d'acceptation

- Étant donné un salaire de 3 000 € (`INCOME`), 1 800 € de dépenses catégorisées, un remboursement
  de 50 € sur « Santé » et un virement de 500 € vers le Livret A en mars, alors : revenus = 3 000 €,
  dépenses = 1 750 €, épargne du mois = 1 250 €, taux d'épargne = 41,7 %. Le virement n'apparaît
  nulle part dans ces chiffres.
- Le patrimoine ne change pas du fait du virement de 500 € (−500 sur le compte courant, +500 sur le livret).
