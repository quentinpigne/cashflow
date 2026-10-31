# Spec 08 — Tableau de bord et indicateurs

Statut : **Validée** (2026-10-20)

## Principe

Chaque indicateur a **une définition écrite ici**, testée, et accessible dans l'interface (info-bulle
« comment est-ce calculé ? »). Aucun chiffre de démonstration : sans donnée, un indicateur affiche
« — » et explique ce qui manque.

## Définitions

Sauf mention contraire, les indicateurs portent sur un **mois civil** M et sur les comptes en EUR
(R-CPT-2). Les virements internes sont exclus des revenus et des dépenses (R-TRX-32). Les flux sont
rattachés au mois par leur date d'analyse (R-TRX-57 : la `date`, sauf mois d'analyse contraire) ; les soldes et
le patrimoine utilisent la date de solde (R-TRX-7).

| Indicateur | Définition | Règle |
|---|---|---|
| **Patrimoine** (net) | Σ des soldes calculés (comptes `CASH` et `FIXED_SAVINGS`) et des valeurs (comptes `VARIABLE_SAVINGS`, R-CPT-6) des comptes `include_in_net_worth`, + Σ des valeurs des biens (R-BIE-2) − Σ des capitaux restants dus des prêts (R-PRE-4) comptés dans le patrimoine, à la date D. Le détail affiche l'actif brut (comptes et biens) et les dettes (prêts) séparément. | R-TDB-1 |
| **Variation de patrimoine** | Patrimoine(fin M) − patrimoine(fin M−1), décomposée en **épargne du mois** (flux) et **effet de marché + intérêts** (le reste) | R-TDB-2 |
| **Revenus** | Σ des ventilations sur des catégories `INCOME`, hors catégories hors analyses (R-TRX-24) | R-TDB-3 |
| **Dépenses** | −Σ des ventilations sur des catégories `EXPENSE`, hors catégories hors analyses (R-TRX-24) (les remboursements viennent en déduction) | R-TDB-4 |
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
- **R-TDB-14** — **Mois à venir de l'évolution** (TDB-03) : les mois qui suivent le mois en cours, jusqu'à
  décembre, affichent des revenus et des dépenses **prévus**, distingués du réel. Revenus prévus = Σ des
  lignes de revenu du budget du mois (R-BUD-50). Dépenses prévues = Σ des lignes de dépense mensuelles et
  des quotes-parts annuelles du mois + Σ des restants prévus du mois (R-BUD-71) des projets exclus des
  budgets du mois et pris en compte (R-BUD-72, interrupteur « Projets planifiés » de l'écran) ; une mise de
  côté n'est pas une dépense et n'y entre pas. **Estimation de fin d'année** = réel du 1er janvier à
  aujourd'hui + **encore à venir du mois en cours** + Σ des prévus des mois suivants, pour les revenus, les
  dépenses et l'épargne. Encore à venir du mois en cours = Σ, ligne de revenu ou de dépense par ligne, de
  (prévu du mois − réel du mois) quand c'est positif — chaque ligne compte ainsi le plus grand de son prévu
  et de son réel, quel que soit le jour où tombent ses mouvements ; une ligne annuelle ne dépasse pas ce qui
  reste de son année — + Σ des restants prévus du mois en cours des projets pris en compte comme ci-dessus.
  Sans ligne de budget, les mois à venir ne montrent que les projets, et l'écran le signale. Un mois à
  venir peut avoir du réel, par des transactions dont le mois d'analyse est futur (R-TRX-57) : chaque mois
  à venir compte donc, comme le mois en cours, son réel + son encore à venir (pour un mois sans réel, c'est
  son prévu, sauf la quote-part d'une ligne annuelle, qui ne dépasse pas ce qui reste de son année).


### Encadrés de l'accueil (TDB-10 à TDB-12)

Chaque encadré ci-dessous est masqué quand il n'a rien à montrer, ou quand sa rubrique est désactivée
(prêts, salaires : spec 09, spec 10). Un clic sur son titre ouvre l'écran détaillé.

- **R-TDB-15** — **Trésorerie à venir** (TDB-10) : la projection de trésorerie (R-PRJ-10 à R-PRJ-12) du
  premier compte courant (`CHECKING`) ouvert, dans l'ordre des comptes (`display_order`, CPT-07), sur **30 jours**, avec la
  dépense variable moyenne (R-PRJ-11) et les projets selon l'interrupteur « Projets planifiés » (R-BUD-72) :
  solde d'aujourd'hui, solde dans 30 jours, solde le plus bas et sa date (signalé sous zéro), mini-courbe.
- **R-TDB-16** — **Prochaines échéances** (TDB-10) : sur les **15 prochains jours**, tous comptes confondus,
  les échéances prévues des récurrences non encore honorées (R-TRX-40) et la prochaine échéance de chaque
  prêt avec tableau (R-PRE-3), par date croissante, 8 au plus : date, libellé, compte ou prêt, montant.
- **R-TDB-17** — **Fin d'année** (TDB-10) : l'estimation de fin d'année (R-TDB-14 : revenus, dépenses,
  épargne), avec l'interrupteur « Projets planifiés », comparée au réel de l'année civile précédente (Σ de ses
  douze mois, R-TDB-3 à R-TDB-6) ; l'écart s'affiche pour chacun des trois chiffres, seulement si l'historique
  couvre toute l'année précédente (premier compte ouvert au plus tard le 1er janvier de cette année).
- **R-TDB-18** — **Projets** (TDB-10) : les projets en cours et, selon l'interrupteur « Projets planifiés »,
  planifiés (R-BUD-72) : réel sur prévu et jauge (R-BUD-22, R-BUD-26), mise de côté du mois en cours s'il y
  en a une (R-BUD-73), et **prochaine dépense prévue** : le premier mois du calendrier prévu (R-BUD-70) à
  partir du mois en cours, avec son montant.
- **R-TDB-19** — **Dépenses par catégorie** (TDB-11) : les **5** catégories de premier niveau aux plus
  fortes dépenses du mois en cours (R-TDB-3, par date d'analyse ; le non catégorisé a son encadré), chacune
  avec sa moyenne sur 12 mois
  (R-TDB-8) ; un clic ouvre les transactions de la catégorie qui comptent dans le mois (R-TRX-58).
- **R-TDB-20** — **Évolution** (TDB-11) : sur les 12 derniers mois, mois en cours compris, l'épargne de
  chaque mois (R-TDB-6) en barres et le taux d'épargne glissant sur 12 mois (R-TDB-7) en courbe.
- **R-TDB-21** — **Tiers du mois** (TDB-11) : les **5** tiers aux plus fortes dépenses du mois en cours
  (R-TIE-20), avec leur part des dépenses du mois rattachées à un tiers ; un clic ouvre la fiche du tiers.
- **R-TDB-22** — **Courbe du patrimoine** (TDB-11) : option de l'encadré Patrimoine, activée par défaut :
  le patrimoine à chaque fin de mois des 12 derniers mois et aujourd'hui (TDB-04), en courbe sous le montant.
- **R-TDB-23** — **Comptes à mettre à jour** (TDB-12) : les comptes ouverts qui ont au moins un import
  validé (spec 03), avec la date de leur dernier relevé importé (fin de période du lot, à défaut sa date de
  validation) et de leur dernier rapprochement (spec 04). Un compte dont le dernier relevé date de plus de
  **31 jours** est **à mettre à jour** : signalé, et listé en premier.
- **R-TDB-24** — **Documents à classer** (TDB-12) : le nombre de documents sans type de la bibliothèque
  (spec 12, R-DOC-20) et les 3 plus récents ; un clic ouvre la bibliothèque filtrée sur « à classer ».
- **R-TDB-25** — **Démarches à relancer** (TDB-12) : les demandes de crédit qui ont au moins une démarche à
  relancer (R-CRE-26), avec leur nombre ; un clic ouvre la demande.
- **R-TDB-26** — **Salaire** (TDB-12) : pour chaque emploi en cours, le dernier mois contrôlé hors mois en
  cours (R-SAL-9) : reçu, attendu et écart, signalé au-delà de la tolérance ; et les mois sans fiche de paie
  (SAL-09). Un clic ouvre la fiche de l'emploi.

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
| TDB-09 | V1 | **Mois à venir et estimation de fin d'année** dans l'évolution (TDB-03), budget et projets compris (R-TDB-14, BUD-14) |
| TDB-10 | V1 | **Accueil, anticiper** : encadrés Trésorerie à venir, Prochaines échéances, Fin d'année et Projets (R-TDB-15 à R-TDB-18) |
| TDB-11 | V1 | **Accueil, analyser** : encadrés Dépenses par catégorie, Évolution et Tiers du mois, courbe du patrimoine (R-TDB-19 à R-TDB-22) |
| TDB-12 | V1 | **Accueil, agir** : encadrés Comptes à mettre à jour, Documents à classer, Démarches à relancer et Salaire (R-TDB-23 à R-TDB-26) |

### TDB-01 — Critères d'acceptation

- Étant donné un salaire de 3 000 € (`INCOME`), 1 800 € de dépenses catégorisées, un remboursement
  de 50 € sur « Santé » et un virement de 500 € vers le Livret A en mars, alors : revenus = 3 000 €,
  dépenses = 1 750 €, épargne du mois = 1 250 €, taux d'épargne = 41,7 %. Le virement n'apparaît
  nulle part dans ces chiffres.
- Le patrimoine ne change pas du fait du virement de 500 € (−500 sur le compte courant, +500 sur le livret).

### TDB-09 — Critères d'acceptation

- Étant donné, le 10 octobre, un salaire prévu de 3 000 € pas encore reçu, des courses prévues à 400 € dont
  100 € faites, des restaurants prévus à 150 € et déjà dépassés (200 €), et une assurance annuelle de
  1 200 € payée en janvier, alors l'encore à venir d'octobre est de 3 000 € de revenus et 300 € de
  dépenses : le dépassement des restaurants ne compense rien, l'assurance n'est plus à venir. Une fois
  le salaire reçu, il n'est plus à venir et l'estimation de fin d'année ne change pas.
