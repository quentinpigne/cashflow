# ADR-0022 — Biens et prêts hors des comptes

- **Statut** : Proposé
- **Date** : 2026-10-22
- **Concerne** : ADR-0019 (remplacé), ADR-0020, ADR-0021, spec 01, spec 02, spec 03, spec 04, spec 05,
  spec 07, spec 08, spec 09 (nouvelle), modèle de données, glossaire, écrans

## Contexte

L'ADR-0019 a fait des biens une famille de comptes (`PROPERTY`), et les prêts en étaient déjà une
(`LIABILITY`). À l'usage, ce rattachement gêne : un bien a une date et un prix d'achat, pas une date
d'ouverture ni un numéro de compte ; un prêt a un capital emprunté, une date de déblocage, des conditions,
pas de relevé. Le formulaire de compte, la liste des comptes et l'accueil doivent masquer ou détourner des
champs, et le capital remboursé passe par une « jambe » sur un compte de prêt (ADR-0021) qui n'existe que
pour ce besoin.

Beaucoup d'utilisateurs n'ont ni bien ni prêt : ces notions ne doivent pas encombrer leur interface.

## Options envisagées

1. **Garder des familles de comptes, mieux habillées** (champs propres selon la famille). ✅ Pas de
   migration lourde. ❌ Le modèle reste faux : un bien ou un prêt n'est pas un contenant d'argent, et chaque
   écran ou règle sur les comptes doit les exclure.
2. **Deux entités à part, `Asset` et `Loan`**, avec leurs rubriques, et une ventilation de remboursement
   rattachée au prêt pour le capital des mensualités. ✅ Chaque notion a ses champs et ses règles ; les
   comptes redeviennent des contenants d'argent. ✅ Rubriques activables. ❌ Migration des données
   existantes, et le patrimoine agrège trois sources au lieu d'une.

## Décision

Option 2.

- **Bien** (`Asset`) : nom, type, date et prix d'achat, estimations datées (`AssetValuation`), vente
  optionnelle. Valeur = dernière estimation, sinon prix d'achat (R-BIE-2).
- **Prêt** (`Loan`) : nom, type, organisme prêteur, date de déblocage, capital emprunté, bien financé
  optionnel, et un **mode de suivi** : avec tableau d'amortissement (conditions de l'ADR-0020) ou suivi
  simple par capitaux restants dus saisis (`LoanBalance`).
- **Capital restant dû d'un prêt avec tableau** : celui du tableau à la date (R-PRE-4), que les mensualités
  soient enregistrées ou non. Il n'y a plus d'écart entre « compte » et « tableau » à expliquer.
- **Mensualité** : sa part de capital devient une **ventilation de remboursement** rattachée au prêt
  (`Split.loan_id`), sans catégorie, exclue des flux comme un virement (R-PRE-7). L'ADR-0021 reste en
  vigueur (un virement relie deux ventilations), mais la mensualité n'en a plus besoin : une jambe de
  virement redevient une transaction à ventilation unique.
- **Patrimoine** = Σ comptes + Σ valeurs des biens − Σ capitaux restants dus des prêts, pour ce qui est
  compté dans le patrimoine (R-TDB-1).
- **Rubriques activables** dans les réglages, désactivables seulement quand elles sont vides (R-BIE-7,
  R-PRE-12). Une nouvelle installation les a désactivées.
- L'ADR-0019 est **remplacé**. L'ADR-0020 reste valable pour le calcul du tableau ; ses conditions
  s'attachent désormais au prêt, et non plus à un compte `LIABILITY`.

## Conséquences

- Les familles de comptes `LIABILITY` et `PROPERTY` disparaissent, avec leurs natures ; R-CPT-12 et
  R-CPT-14 à R-CPT-17, CPT-09 et CPT-10 sont supprimées au profit de la spec 09.
- **Migration des données** :
  - chaque compte `PROPERTY` devient un bien (date d'achat = date d'ouverture, prix = solde d'ouverture,
    valorisations → estimations) ;
  - chaque compte `LIABILITY` devient un prêt : avec ses conditions s'il en a (mode tableau, capital et
    première échéance repris), sinon en suivi simple (capital emprunté = capital restant dû d'ouverture,
    valorisations → capitaux restants dus saisis) ;
  - chaque mensualité ventilée garde ses ventilations : la ventilation de virement du capital devient une
    ventilation de remboursement, et la jambe sur le compte de prêt disparaît ;
  - toute autre transaction d'un compte de prêt ou de bien bloque la migration : à vérifier sur une copie
    de la base de production avant le déploiement ;
  - les rubriques sont activées si elles contiennent des données ; les comptes convertis sont supprimés.
- Nouvelles tables, nouvelle colonne `split.loan_id`, table `account` recréée (contrainte des natures) ;
  export (ADR-0008) étendu.
- Nouveaux écrans : rubriques Biens et Prêts, fiche d'un bien, fiche d'un prêt, interrupteurs dans les
  réglages ; l'accueil gagne des encadrés Biens et Prêts, masqués si la rubrique est désactivée.
