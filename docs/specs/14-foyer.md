# Spec 14 — Foyer

Statut : **Validée** (2026-10-31)
Modèle : [Member, AccountOwner, AssetOwner, LoanOwner](../modele-de-donnees.md#foyer--spec-14)
Décision : [ADR-0006](../adr/0006-mono-utilisateur.md) (membres sans connexion)

## Concepts

Le **foyer** réunit les **membres** de la famille : moi, mon conjoint, les enfants. Un membre est une
**personne**, pas un utilisateur : il n'a ni identifiant ni mot de passe, et l'application garde une seule
connexion (ADR-0006). Les membres servent à savoir **à qui** appartiennent les comptes, les biens, les prêts,
les emplois et les documents, pour voir le patrimoine **par membre** et pas seulement pour le foyer.

Un compte joint, une maison achetée à deux ou un prêt immobilier commun ont **plusieurs propriétaires**,
chacun avec sa **quote-part** :

```
Maison de Meylan (bien, 420 000 €)     Prêt immobilier (180 000 € restant dus)
 ├─ Quentin  60 %  → 252 000 €          ├─ Quentin  50 %  → 90 000 €
 └─ Claire   40 %  → 168 000 €          └─ Claire   50 %  → 90 000 €
```

Une **transaction** peut aussi concerner un membre : les chaussures de Léo, le salaire de Claire, le forfait
téléphone de Quentin payé depuis le compte joint. Le membre se pose sur la **ventilation**, indépendamment des
propriétaires du compte qui paie.

Ce qui reste commun au foyer ne change pas : catégories, règles, tiers, budgets, projets, scénarios de
projection et récurrences n'ont pas de propriétaire (Q19).

## Membres

- **R-MBR-1** — Un **membre** a un nom (≤ 40 caractères), unique sans distinction de casse, et
  optionnellement une couleur (`#RRGGBB`). Les membres sont rangés dans un ordre choisi (`display_order`).
- **R-MBR-2** — **Supprimer un membre** est toujours possible, après une confirmation qui indique le nombre
  d'objets concernés. Ses quotes-parts sont retirées : un objet qu'il possédait avec d'autres voit sa
  quote-part répartie entre les autres propriétaires au prorata de leurs quotes-parts (R-MBR-5) ; un objet
  qu'il possédait seul n'a plus de propriétaire. Ses emplois et ses ventilations n'ont plus de membre, ses
  documents perdent ce rattachement (R-DOC-12). Aucun montant ne change.
- **R-MBR-3** — La rubrique **Foyer** s'active dans les réglages, comme Biens, Prêts et Salaires (R-BIE-7) :
  désactivable seulement s'il n'existe aucun membre ; désactivée, l'interface n'en montre rien (réglages,
  formulaires, sélecteur de vue, filtres) et tout se lit comme aujourd'hui, pour le foyer. Une nouvelle
  installation l'a désactivée.

## Propriétaires et quotes-parts

- **R-MBR-4** — Un **compte**, un **bien** ou un **prêt** a zéro, un ou plusieurs **propriétaires** (pour un
  prêt : les emprunteurs), chacun au plus une fois, avec une **quote-part** en centièmes de pour cent, de 1 à
  10 000 (100 %). Dès qu'il a un propriétaire, la somme des quotes-parts vaut **exactement 100 %**. Un objet
  sans propriétaire est **non attribué** : il compte pour le foyer, pour aucun membre.
- **R-MBR-5** — À la saisie, ajouter un propriétaire **répartit à parts égales** (deux propriétaires : 50 % et
  50 % ; trois : 33,34 %, 33,33 %, 33,33 %, le premier recevant le reste) ; chaque quote-part se modifie
  ensuite. Toute répartition au prorata (R-MBR-2) se fait au **plus fort reste**, en centièmes de pour cent,
  pour garder une somme de 100 %.
- **R-MBR-6** — Les propriétaires d'un compte, d'un bien ou d'un prêt se modifient à tout moment, même sur un
  compte qui a des transactions rapprochées : ils n'affectent aucun solde (R-TRX-3). Ils valent pour **toute
  la durée** de l'objet : il n'y a pas d'historique des quotes-parts.
- **R-MBR-7** — Un **emploi** appartient à **au plus un** membre, sans quote-part (Q21). Ses rémunérations,
  primes et fiches de paie le suivent.
- **R-MBR-8** — Un **document** peut être rattaché à un ou plusieurs membres (pièce d'identité, avis
  d'imposition), comme à un compte ou à un tiers : le membre est une cible de rattachement de plus (R-DOC-9).

## Transactions d'un membre

- **R-MBR-14** — Une **ventilation** est affectée à **au plus un** membre : celui que la dépense ou le revenu
  concerne. Sans membre, elle concerne le foyer. Le membre ne dépend pas des propriétaires du compte (un achat
  pour Léo payé depuis le compte joint). Une **ventilation de virement** n'a jamais de membre : un virement
  n'est ni une dépense ni un revenu (ADR-0004, R-TRX-32).
- **R-MBR-15** — Affecter une **transaction** à un membre affecte toutes ses ventilations, sauf celle d'un
  virement. Une transaction dont les ventilations ont des membres différents affiche « Plusieurs membres ».
  Le membre se choisit à la saisie, dans le détail d'une transaction (par ventilation) et en masse depuis la
  liste des transactions. Il reste modifiable sur une transaction rapprochée : il n'affecte aucun solde
  (R-TRX-3).
- **R-MBR-16** — Quand des ventilations sont recalculées sans que l'utilisateur les saisisse (réapplication des
  règles de catégorisation, IMP-08), le membre est conservé si toutes les anciennes ventilations avaient le
  même : les nouvelles le reçoivent.
- **R-MBR-17** — La liste des transactions se **filtre par membre** : celles qui ont au moins une ventilation
  affectée à ce membre ; ou « Sans membre » : celles qui ont au moins une ventilation hors virement sans
  membre.

## Patrimoine par membre

- **R-MBR-9** — La **part d'un membre** dans un montant (solde, valeur, capital restant dû) est ce montant
  multiplié par sa quote-part, en unités mineures. Les parts des propriétaires d'un même montant sont
  arrondies au **plus fort reste** (à reste égal, le premier dans l'ordre des membres) : leur somme égale
  exactement le montant (ADR-0001).
- **R-MBR-10** — Le **patrimoine d'un membre** à une date D reprend la définition du patrimoine (R-TDB-1)
  restreinte à ses parts : Σ de ses parts des comptes et des biens comptés dans le patrimoine − Σ de ses
  parts des capitaux restants dus des prêts comptés dans le patrimoine. Les comptes en devise autre que l'euro
  restent exclus (R-CPT-2).
- **R-MBR-11** — Invariant : à toute date, Σ des patrimoines des membres + patrimoine **non attribué**
  (objets sans propriétaire) = patrimoine du foyer.
- **R-MBR-12** — La **vue** se choisit entre **Foyer** (défaut, comme aujourd'hui) et un membre, sur l'encadré
  Patrimoine de l'accueil et sur l'onglet Patrimoine des projections (TDB-04). En vue membre, le montant, le détail par famille
  et la courbe des 12 derniers mois sont ceux du membre (R-MBR-10) ; un compte, un bien ou un prêt partagé
  affiche sa part et sa quote-part. La **projection** reste celle du foyer : elle n'est pas affichée en vue
  membre. La vue choisie est mémorisée sur l'appareil.
- **R-MBR-13** — La liste des comptes se **filtre par membre** : les comptes dont il est propriétaire, avec
  leur solde entier et sa part. Les budgets, les analyses de dépenses et de revenus et les projections ne
  changent pas : ils restent ceux du foyer (Q19).

## Features

| ID | Priorité | Feature |
|---|---|---|
| MBR-01 | V2 | Gérer les membres du foyer (création, renommage, couleur, ordre, suppression) ; activation de la rubrique |
| MBR-02 | V2 | Propriétaires et quotes-parts des comptes, des biens et des prêts |
| MBR-03 | V2 | Membre d'un emploi ; documents rattachés à un membre |
| MBR-04 | V2 | Patrimoine par membre (accueil, onglet Patrimoine des projections) et filtre des comptes par membre |
| MBR-05 | V2 | Dépenses, revenus et épargne par membre dans les analyses et le budget (à concevoir : à partir des membres des ventilations, R-MBR-14) |
| MBR-06 | V2 | Membre d'une transaction ou d'une ventilation, filtre et affectation en masse (R-MBR-14 à R-MBR-17) |

### MBR-01 — Critères d'acceptation

- Étant donné la rubrique Foyer désactivée, alors aucun écran ni aucun champ ne parle de membre, et le
  patrimoine est celui d'aujourd'hui (R-MBR-3).
- Étant donné le membre « Claire », quand je crée « CLAIRE », alors la création est refusée (R-MBR-1).
- Étant donné deux membres, quand je désactive la rubrique, alors c'est refusé tant qu'ils existent (R-MBR-3).
- Étant donné le compte joint de Quentin (50 %) et Claire (50 %) et le livret de Claire seule, quand je
  supprime Claire, alors le compte joint appartient à Quentin à 100 % et le livret n'a plus de propriétaire
  (R-MBR-2).
- Étant donné un bien à Quentin (50 %), Claire (30 %) et Léo (20 %), quand je supprime Quentin, alors Claire a
  60 % et Léo 40 % (R-MBR-2, R-MBR-5).

### MBR-02 — Critères d'acceptation

- Étant donné un compte, quand j'y ajoute Quentin puis Claire, alors chacun a 50 % (R-MBR-5).
- Étant donné un compte à Quentin 60 % et Claire 30 %, quand j'enregistre, alors c'est refusé : la somme ne
  vaut pas 100 % (R-MBR-4).
- Étant donné un compte rapproché, quand je change ses propriétaires, alors c'est accepté sans déverrouillage
  (R-MBR-6).

### MBR-06 — Critères d'acceptation

- Étant donné une dépense de 80 € sur le compte joint, quand je l'affecte à Léo, alors sa ventilation a le
  membre Léo et le solde ne change pas (R-MBR-14).
- Étant donné une dépense de 100 € ventilée 60 € Alimentation et 40 € Habillement, quand j'affecte 40 € à Léo,
  alors la liste filtrée sur Léo la montre, et la liste filtrée sur « Sans membre » aussi (R-MBR-17) ; la
  transaction affiche « Plusieurs membres » dès que les 60 € sont affectés à Claire (R-MBR-15).
- Étant donné un virement et une dépense sélectionnés, quand je les affecte à Claire en masse, alors seule la
  dépense est affectée ; le virement est compté comme ignoré (R-MBR-14).
- Étant donné une dépense rapprochée, quand je change son membre, alors c'est accepté sans déverrouillage
  (R-MBR-15).
- Étant donné une dépense affectée à Léo, quand je réapplique les règles et qu'une règle la ventile autrement,
  alors les nouvelles ventilations sont affectées à Léo (R-MBR-16).
- Étant donné Léo, quand je le supprime, alors ses ventilations n'ont plus de membre et aucun montant ne change
  (R-MBR-2).

### MBR-04 — Critères d'acceptation

- Étant donné le compte joint (2 000,01 €, Quentin 50 %, Claire 50 %), la maison (420 000 €, Quentin 60 %,
  Claire 40 %), le prêt (180 000 € restant dus, 50/50) et un livret non attribué (5 000 €), alors : Quentin =
  1 000,01 + 252 000 − 90 000 = 163 000,01 € ; Claire = 1 000,00 + 168 000 − 90 000 = 79 000,00 € ; non attribué
  = 5 000 € ; foyer = 247 000,01 € (R-MBR-9 à R-MBR-11).
- Étant donné la vue « Claire » choisie sur l'accueil, quand je reviens sur l'accueil, alors la vue « Claire »
  est toujours affichée (R-MBR-12).
