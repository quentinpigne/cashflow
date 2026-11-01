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
ensuite les membres à afficher : encadré par encadré sur l'accueil, et par des cases à cocher dans les analyses
et les projections.

```
Cases cochées : [x] Foyer  [x] Quentin  [ ] Claire

Compte courant de Quentin ─ courses (sans membre)         → Quentin  ✔
                          └ cadeau pour Claire (Claire)   → Claire   ✘
Compte joint (sans membre) ─ cantine de Léo (sans membre) → Foyer    ✔
Livret de Claire                                           → Claire   ✘
```

Un compte a **au plus un** membre : il n'y a pas de compte joint à quotes-parts, un compte commun reste au
foyer. Un **bien** se répartit entre des membres par quotes-parts, le reste étant au foyer (la maison à 60 % à
Quentin, 40 % à Claire). Un **document** est affecté à un ou plusieurs membres, ou à personne. Les prêts n'ont pas de
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
  transactions rapprochées : il n'affecte aucun solde (R-TRX-3). Changer le titulaire d'un compte qui a des
  transactions demande s'il faut aussi passer au nouveau titulaire les ventilations du compte affectées à
  l'ancien (ou au foyer, si le compte n'en avait pas) ; sinon elles restent telles quelles.
- **R-MBR-5** — Une **ventilation** est affectée à **un membre** ou au **foyer** (sans membre) : celui que la
  dépense ou le revenu concerne (le cadeau pour Claire, la cantine de Léo, les courses communes). Affecter une **transaction** à un membre affecte toutes
  ses ventilations ; ventilée, chaque ventilation se choisit séparément. Plusieurs transactions s'affectent
  d'un coup depuis la liste des transactions, les virements étant laissés tels quels. Une **ventilation de virement** n'a
  jamais de membre : un virement n'est ni une dépense ni un revenu (ADR-0004). Le membre reste modifiable sur
  une transaction rapprochée (R-TRX-3). Quand la réapplication des règles (IMP-08) recalcule les ventilations,
  les nouvelles reçoivent le membre commun aux anciennes, à défaut le titulaire du compte.
- **R-MBR-6** — À la **création** d'une ventilation, son membre est par défaut le **titulaire du compte**
  (le foyer si le compte n'en a pas) : saisie (le champ est prérempli), import, récurrence, intérêts, échéance
  de prêt. Il se change ensuite librement, y compris pour le foyer. Ainsi, les dépenses du compte personnel de
  Quentin sont à Quentin sans rien affecter, et une dépense commune payée depuis ce compte s'affecte au foyer.
  Dans les listes, une transaction n'affiche son membre que s'il diffère du titulaire du compte.
- **R-MBR-7** — Un **emploi** est affecté à **au plus un** membre (Q21). Ses rémunérations, primes et fiches
  de paie le suivent. Sans membre, il est au foyer.

## Choix des membres affichés

- **R-MBR-8** — Le **choix des membres** se fait par des **cases à cocher** : « Foyer » puis un membre par
  membre, dans leur ordre. Toutes sont cochées par défaut ; au moins une le reste. Le choix est mémorisé sur
  l'appareil. Il se fait **encadré par encadré** sur l'accueil, pour les encadrés concernés (R-MBR-9) : un même
  accueil peut montrer le patrimoine du foyer à côté des dépenses de Claire. Les **analyses** et les
  **projections** ont chacune un choix pour tout l'écran.
  Ne sont proposées que les cases qui **comptent quelque chose** pour l'encadré ou l'écran : un membre sans
  compte n'est pas proposé pour le patrimoine, « Foyer » ne l'est pas quand tous les comptes ont un titulaire
  (et qu'il n'y a ni bien ni prêt). Pour le patrimoine, un membre est titulaire d'un compte compté dans le
  patrimoine ; pour les comptes, d'un compte ouvert ; pour la trésorerie, d'un compte courant ouvert ; pour
  l'épargne, d'un compte d'épargne ou d'épargne de précaution ouvert ; pour les flux, d'un compte ou d'une
  ventilation ; pour le salaire, d'un emploi. « Foyer » est proposé quand un tel objet n'a pas de membre (et,
  pour le patrimoine, quand un bien n'est pas réparti en entier ou qu'il existe un prêt). Pour le patrimoine, un
  membre qui a une quote-part d'un bien est aussi proposé. S'il ne reste qu'une case, il n'y a pas de choix.
  Une case non proposée ne compte rien : les chiffres n'en dépendent pas.
- **R-MBR-9** — Avec un choix de membres :
  - les **flux** (revenus, dépenses, non catégorisé, épargne, par catégorie, par tiers, évolution) ne
    comptent que les ventilations dont le membre (ou « Foyer », sans membre) est coché ;
  - les **soldes**, le **patrimoine** et les **projections** (patrimoine, trésorerie, épargne) ne comptent
    que les comptes dont le membre est coché, les comptes sans membre avec « Foyer » ; d'un bien, la part de
    chaque membre coché, et le reste avec « Foyer » (R-MBR-12) ; les prêts comptent avec « Foyer » ;
  - l'encadré **Salaire** ne montre que les emplois dont le membre est coché, ceux sans membre avec « Foyer » ;
  - l'encadré **Épargne** ne compte que les comptes dont le titulaire est coché (épargne de précaution,
    intérêts, épargne variable), et des dépenses mensuelles moyennes (R-TDB-8) que les ventilations dont le
    membre est coché.
  - Sur l'accueil, les encadrés concernés sont donc Patrimoine, Mois en cours, Comptes, Trésorerie à venir,
    Dépenses par catégorie, Évolution, Tiers du mois, Salaire et Épargne. Les autres (budget, projets, fin
    d'année, échéances, à catégoriser, biens, prêts, comptes à mettre à jour, documents, démarches…) restent
    ceux du foyer et n'offrent pas de choix.
  - Ce qui vient du budget ne se répartit pas entre membres : dans les analyses, quand toutes les cases ne
    sont pas cochées, les mois à venir de l'évolution (R-TDB-14) et l'estimation de fin d'année ne sont pas
    affichés, et l'écran le dit.
- **R-MBR-10** — Invariant : toutes les cases cochées, chaque chiffre est celui d'aujourd'hui ; et pour un
  flux, la somme des chiffres de chaque case cochée seule égale le chiffre de toutes les cases.

## Biens répartis entre membres

- **R-MBR-11** — Un **bien** se répartit entre **zéro, un ou plusieurs** membres, chacun avec sa **quote-part**,
  en centièmes de pour cent (de 0,01 % à 100 %). La somme des quotes-parts ne dépasse pas 100 % ; le **reste est
  au foyer**. Sans quote-part, le bien est entièrement au foyer, comme aujourd'hui. La quote-part se saisit en
  pourcentage ou en **montant** : un montant est converti en quote-part d'après la valeur du bien au jour de la
  saisie (arrondie au centième de pour cent), car c'est la quote-part qui reste quand la valeur change. Elle
  se modifie à tout moment ; elle vaut pour toute la durée du bien (pas d'historique).
- **R-MBR-12** — La **part d'un membre** dans la valeur d'un bien est cette valeur multipliée par sa
  quote-part ; le foyer a le reste. Les parts sont arrondies au centime au **plus fort reste** (à reste égal,
  le premier dans l'ordre des membres, le foyer en dernier), si bien que leur somme égale exactement la valeur
  (ADR-0001). Supprimer un membre rend sa quote-part au foyer (R-MBR-2).

## Documents d'un membre

- **R-MBR-13** — Un **document** est affecté à **zéro, un ou plusieurs** membres (l'avis d'imposition du
  couple) ; sans membre, il ne concerne **personne** ; il n'est jamais affecté au « foyer ». Ses membres se
  modifient à tout moment ; supprimer un membre le retire de ses documents (R-MBR-2). La bibliothèque se filtre
  par membre (les documents de ce membre, ou « Personne »).
- **R-MBR-14** — À sa création, un **relevé de compte** est affecté au titulaire du compte auquel il est
  rattaché (envoyé depuis le compte, ou archivé à l'import d'un relevé PDF, R-IMP-28), une **fiche de paie** au
  membre de l'emploi ; les autres documents, à personne. Ce n'est qu'une valeur par défaut : elle ne suit pas
  les changements de titulaire ou de membre de l'emploi.

## Features

| ID | Priorité | Feature |
|---|---|---|
| MBR-01 | V2 | Gérer les membres du foyer (création, modification, ordre, suppression) ; activation de la rubrique |
| MBR-02 | V2 | Membre d'un compte et d'un emploi |
| MBR-03 | V2 | Membre d'une transaction ou de chaque ventilation |
| MBR-04 | V2 | Choix des membres affichés dans l'accueil, les analyses et les projections |
| MBR-05 | V2 | Comptes joints à quotes-parts ; membres des prêts (à concevoir) |
| MBR-06 | V2 | Biens répartis entre membres par quotes-parts, en pourcentage ou en montant |
| MBR-07 | V2 | Membres d'un document, par défaut celui du compte ou de l'emploi |

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
- Étant donné le compte de Quentin, quand je saisis une dépense, alors le champ Membre propose Quentin ; quand
  je choisis « Foyer », alors la dépense compte pour le foyer, et la liste affiche « Foyer » après son libellé
  (R-MBR-6).
- Étant donné le compte joint sans titulaire et ses transactions au foyer, quand j'en fais le compte de Claire
  et que j'accepte de passer ses ventilations, alors elles sont à Claire ; si je refuse, elles restent au
  foyer (R-MBR-4).

### MBR-04 — Critères d'acceptation

- Étant donné le compte courant de Quentin, le livret de Claire et le compte joint sans membre, quand je ne
  coche que Claire, alors le patrimoine ne compte que le livret de Claire (R-MBR-9).
- Étant donné, sur le compte de Quentin, 50 € de courses (membre par défaut : Quentin) et 30 € de cadeau
  affectés à Claire, et sur le compte joint 20 € de cantine (au foyer) : en ne cochant que Quentin, les dépenses valent 50 € ; que
  Claire, 30 € ; que Foyer, 20 € ; tout coché, 100 € (R-MBR-6, R-MBR-10).
- Étant donné Claire seule cochée dans l'encadré Mois en cours, alors l'encadré Patrimoine reste celui du
  foyer ; quand je reviens sur l'accueil, Claire seule est toujours cochée dans Mois en cours, et son titre
  porte « · Claire » (R-MBR-8).
- Étant donné une case décochée dans les analyses, alors l'estimation de fin d'année n'est pas affichée
  (R-MBR-9).

### MBR-06 — Critères d'acceptation

- Étant donné la maison (420 000 €), quand je donne 60 % à Quentin et 30 % à Claire, alors le patrimoine de
  Quentin compte 252 000 €, celui de Claire 126 000 €, et « Foyer » 42 000 € (R-MBR-11, R-MBR-12).
- Étant donné la maison à 420 000 €, quand je saisis 105 000 € pour Claire, alors sa quote-part est 25 % ;
  quand la maison vaut ensuite 440 000 €, sa part vaut 110 000 € (R-MBR-11).
- Étant donné une valeur de 100 000,01 € à 50 / 50, alors les parts valent 50 000,01 € et 50 000,00 € (R-MBR-12).
- Étant donné 70 % à Quentin, quand je donne 40 % à Claire, alors c'est refusé : la somme dépasse 100 %
  (R-MBR-11).

### MBR-07 — Critères d'acceptation

- Étant donné le compte de Quentin, quand j'y envoie un relevé, alors il est affecté à Quentin ; une fiche de
  paie de l'emploi de Claire est affectée à Claire ; une facture envoyée depuis la bibliothèque, à personne
  (R-MBR-14).
- Étant donné un relevé affecté à Quentin, quand je retire Quentin, alors il n'a plus de membre, et le filtre
  « Personne » de la bibliothèque le montre (R-MBR-13).
- Étant donné l'avis d'imposition, quand je l'affecte à Quentin et à Claire, alors il apparaît avec le filtre
  Quentin comme avec le filtre Claire (R-MBR-13).

