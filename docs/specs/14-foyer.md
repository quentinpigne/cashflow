# Spec 14 — Foyer

Statut : **Validée** (2026-10-31)
Modèle : [Member](../modele-de-donnees.md#foyer--spec-14), `Account.member_id`, `Split.member_id`,
`Employment.member_id`
Décision : [ADR-0006](../adr/0006-mono-utilisateur.md) (membres sans connexion)

## Concepts

Le **foyer** réunit les **membres** de la famille : moi, mon conjoint, les enfants. Un membre est une
**personne**, pas un utilisateur : il n'a ni identifiant ni mot de passe, et l'application garde une seule
connexion (ADR-0006).

On affecte à un membre un **compte**, une **transaction** (ou chacune de ses ventilations) et un **emploi**.
Ce qui n'est affecté à personne appartient au **foyer** lui-même, c'est-à-dire à tout le monde. On choisit
ensuite, par des cases à cocher, les membres à afficher dans l'accueil, les analyses et les projections.

```
Cases cochées : [x] Foyer  [x] Quentin  [ ] Claire

Compte courant de Quentin ─ courses (sans membre)         → Quentin  ✔
                          └ cadeau pour Claire (Claire)   → Claire   ✘
Compte joint (sans membre) ─ cantine de Léo (sans membre) → Foyer    ✔
Livret de Claire                                           → Claire   ✘
```

Cette première version est volontairement simple : un compte a **au plus un** membre. Il n'y a pas de compte
joint à quotes-parts : un compte commun reste au foyer. Les biens, les prêts et les documents n'ont pas de
membre. Catégories, règles, tiers, budgets, projets, scénarios de projection et récurrences restent communs
au foyer (Q19).

## Membres

- **R-MBR-1** — Un **membre** a un nom (≤ 40 caractères), unique sans distinction de casse, et
  optionnellement une couleur (`#RRGGBB`). Les membres sont rangés dans un ordre choisi (`display_order`) ;
  un nouveau membre se range après le dernier.
- **R-MBR-2** — **Supprimer un membre** est toujours possible, après une confirmation qui indique le nombre
  de comptes, de ventilations et d'emplois qui lui sont affectés. Ils reviennent au foyer (plus de membre).
  Aucun montant ne change.
- **R-MBR-3** — La rubrique **Foyer** s'active dans les réglages, comme Biens, Prêts et Salaires (R-BIE-7) :
  désactivable seulement s'il n'existe aucun membre ; désactivée, l'interface n'en montre rien (réglages,
  champs Membre, cases à cocher) et tout se lit comme aujourd'hui. Une nouvelle installation l'a désactivée.

## Affectations

- **R-MBR-4** — Un **compte** est affecté à **au plus un** membre, son titulaire. Sans membre, c'est un
  compte du foyer (un compte joint, par exemple). Le membre se modifie à tout moment, même si le compte a des
  transactions rapprochées : il n'affecte aucun solde (R-TRX-3).
- **R-MBR-5** — Une **ventilation** est affectée à **au plus un** membre : celui que la dépense ou le revenu
  concerne (le cadeau pour Claire, la cantine de Léo). Affecter une **transaction** à un membre affecte toutes
  ses ventilations ; ventilée, chaque ventilation se choisit séparément. Une **ventilation de virement** n'a
  jamais de membre : un virement n'est ni une dépense ni un revenu (ADR-0004). Le membre reste modifiable sur
  une transaction rapprochée (R-TRX-3). Quand la réapplication des règles (IMP-08) recalcule les ventilations,
  les nouvelles reçoivent le membre commun aux anciennes, s'il y en a un.
- **R-MBR-6** — Le **membre retenu** d'une ventilation est son membre ; à défaut, le membre de son compte ;
  à défaut, le foyer. Ainsi, les dépenses du compte personnel de Quentin sont à Quentin sans rien affecter,
  et celles du compte joint au foyer, sauf affectation contraire.
- **R-MBR-7** — Un **emploi** est affecté à **au plus un** membre (Q21). Ses rémunérations, primes et fiches
  de paie le suivent. Sans membre, il est au foyer.

## Choix des membres affichés

- **R-MBR-8** — L'**accueil**, les **analyses** et les **projections** ont chacun des **cases à cocher** :
  « Foyer » puis un membre par membre, dans leur ordre. Toutes sont cochées par défaut ; au moins une le
  reste. Le choix est mémorisé sur l'appareil, écran par écran.
- **R-MBR-9** — Avec un choix de membres :
  - les **flux** (revenus, dépenses, non catégorisé, épargne, par catégorie, par tiers, évolution) ne
    comptent que les ventilations dont le membre retenu (R-MBR-6) est coché ;
  - les **soldes**, le **patrimoine** et les **projections** (patrimoine, trésorerie, épargne) ne comptent
    que les comptes dont le membre est coché, les comptes sans membre avec « Foyer » ; les biens et les prêts
    comptent avec « Foyer » ;
  - l'encadré **Salaire** ne montre que les emplois dont le membre est coché, ceux sans membre avec « Foyer ».
  - Ce qui vient du budget ne se répartit pas entre membres : quand toutes les cases ne sont pas cochées, les
    mois à venir de l'évolution (R-TDB-14) et l'encadré Fin d'année ne sont pas affichés, et l'écran le dit.
  - Les autres encadrés et écrans (budget, projets, prochaines échéances, comptes à mettre à jour,
    documents, démarches, transactions) ne changent pas.
- **R-MBR-10** — Invariant : toutes les cases cochées, chaque chiffre est celui d'aujourd'hui ; et pour un
  flux, la somme des chiffres de chaque case cochée seule égale le chiffre de toutes les cases.

## Features

| ID | Priorité | Feature |
|---|---|---|
| MBR-01 | V2 | Gérer les membres du foyer (création, modification, ordre, suppression) ; activation de la rubrique |
| MBR-02 | V2 | Membre d'un compte et d'un emploi |
| MBR-03 | V2 | Membre d'une transaction ou de chaque ventilation |
| MBR-04 | V2 | Choix des membres affichés dans l'accueil, les analyses et les projections |
| MBR-05 | V2 | Comptes joints à quotes-parts ; membres des biens, des prêts et des documents (à concevoir) |

### MBR-01 — Critères d'acceptation

- Étant donné la rubrique Foyer désactivée, alors aucun écran ni aucun champ ne parle de membre (R-MBR-3).
- Étant donné le membre « Claire », quand je crée « CLAIRE », alors la création est refusée (R-MBR-1).
- Étant donné deux membres, quand je désactive la rubrique, alors c'est refusé tant qu'ils existent (R-MBR-3).
- Étant donné le livret de Claire, une dépense affectée à Claire et son emploi, quand je supprime Claire,
  alors le livret, la dépense et l'emploi n'ont plus de membre, et aucun solde ne change (R-MBR-2).

### MBR-02 et MBR-03 — Critères d'acceptation

- Étant donné un compte rapproché, quand je l'affecte à Quentin, alors c'est accepté sans déverrouillage
  (R-MBR-4).
- Étant donné une dépense de 100 € ventilée 60 € Alimentation et 40 € Habillement, quand j'affecte les 40 € à
  Léo, alors seule cette ventilation a un membre (R-MBR-5).
- Étant donné un virement, alors sa ventilation n'accepte pas de membre (R-MBR-5).

### MBR-04 — Critères d'acceptation

- Étant donné le compte courant de Quentin, le livret de Claire et le compte joint sans membre, quand je ne
  coche que Claire, alors le patrimoine ne compte que le livret de Claire (R-MBR-9).
- Étant donné, sur le compte de Quentin, 50 € de courses sans membre et 30 € de cadeau affectés à Claire, et
  sur le compte joint 20 € de cantine sans membre : en ne cochant que Quentin, les dépenses valent 50 € ; que
  Claire, 30 € ; que Foyer, 20 € ; tout coché, 100 € (R-MBR-6, R-MBR-10).
- Étant donné Claire seule cochée sur l'accueil, quand je reviens sur l'accueil, alors seule Claire est
  toujours cochée (R-MBR-8).
- Étant donné une case décochée, alors l'encadré Fin d'année n'est pas affiché (R-MBR-9).
