# ADR-0029 — Adresse portée par une transaction

- **Statut** : Accepté
- **Date** : 2026-11-05
- **Concerne** : spec 02, spec 11, modèle de données, écrans (E7, E14, E15), export ; complète l'ADR-0025

## Contexte

L'ADR-0025 place le lieu d'une dépense sur son **tiers** et écarte un lieu porté par la transaction (« à saisir
à chaque transaction »). À l'usage, on ne veut pas toujours créer un tiers pour localiser une dépense : un
restaurant de vacances, un marché, un péage, un achat ponctuel. Ces transactions sortent alors de l'analyse par
lieu, qui ne connaît que les transactions avec un tiers (R-TIE-28). La base de production contient déjà des
données réelles : l'ADR-0025 n'est pas réécrit, il est complété.

## Options envisagées

1. **Créer un tiers pour chaque lieu** (statu quo) — ✅ rien à changer / ❌ des dizaines de tiers d'un jour, qui
   encombrent la liste des tiers et leurs analyses.
2. **Adresse facultative sur la transaction, prioritaire sur celle du tiers** — ✅ localise une dépense sans
   tiers, ou un tiers itinérant (une enseigne à plusieurs magasins) / ❌ deux sources de lieu à expliquer.
3. **Adresse sur la transaction, utilisée seulement sans tiers** — ✅ une seule source par transaction / ❌ une
   enseigne nationale garde un lieu faux pour un achat fait ailleurs.

Pour le stockage :

1. **Colonnes sur `ledger_transaction`** — ❌ six colonnes presque toujours vides, et des contraintes croisées
   qui imposent de recréer la plus grosse table de la base (ADR-0017).
2. **Table `transaction_address`, au plus une ligne par transaction** — ✅ migration sans recréation, mêmes
   `CHECK` que l'adresse d'un tiers, suppression en cascade avec la transaction.

## Décision

- Une transaction a **optionnellement sa propre adresse**, de même forme et mêmes règles que celle d'un tiers
  (ADR-0025, R-TIE-23 à R-TIE-26) : ligne, code postal, ville, pays, coordonnées facultatives issues de la
  recherche d'adresse. Elle est stockée dans la table `transaction_address` (option 2 du stockage).
- Le **lieu** d'une transaction est **son adresse si elle en a une, sinon celle de son tiers** (option 2). Les
  analyses par lieu et la carte suivent le lieu : une transaction sans tiers mais avec une adresse y entre.
- Une jambe de virement n'a pas d'adresse, comme elle n'a pas de tiers : un virement n'est ni une dépense ni un
  revenu (ADR-0004).
- L'adresse ne change aucun solde : elle reste modifiable sur une transaction rapprochée.
- Elle se saisit sur la fiche d'une transaction, ou sur une sélection depuis la liste des transactions.

## Conséquences

- L'analyse par lieu regroupe des transactions et non plus seulement des tiers : un lieu se déplie en tiers et
  en « Sans tiers » ; un même tiers peut apparaître dans deux lieux si certaines de ses transactions ont leur
  propre adresse.
- La carte place les tiers localisés et les adresses localisées des transactions ; le compte « hors de la
  carte » porte sur les lieux (tiers ou adresses de transaction).
- L'export gagne la table `transaction_address` (ADR-0008).
- Changer l'adresse d'un tiers ne déplace pas les transactions qui ont leur propre adresse.
