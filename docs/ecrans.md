# Écrans — phase 1

Statut : **Validée** (2026-10-20)

Navigation et écrans de la phase 1 de la [roadmap](roadmap.md) (registre exact). Les règles métier
restent dans les specs : ce document décrit **ce qu'on voit et ce qu'on peut faire**, écran par écran,
et renvoie aux features (`CPT-01`, `TRX-03`…). Les écrans des phases suivantes (import,
rapprochement, budgets, épargne, projections) seront ajoutés au moment de leur conception.

Composants : Angular Material (ADR-0009). Les noms entre crochets (`[mat-table]`) indiquent le
composant visé, pour guider l'implémentation.

## Principes

- **Mobile d'abord** pour la consultation et la saisie rapide ; l'ordinateur affiche plus de colonnes
  et garde la navigation visible en permanence (architecture.md, § Interface).
- Deux mises en page, un seul composant par écran : la bascule se fait à **600 px** de large
  (`BreakpointObserver` du CDK).
- **Montants** : format français (`1 234,56 €`), alignés à droite dans les tableaux, en rouge pour une
  sortie, sans signe `+` pour une entrée. Saisie toujours positive, le sens est choisi à part
  (dépense ou revenu).
- **Dates** : `12 mars 2026` dans les listes, `12/03/2026` dans les champs `[mat-datepicker]`, locale
  `fr-FR`, semaine commençant le lundi.
- **Erreurs métier** : le message de l'API (ADR-0016) s'affiche sous le champ concerné, ou dans une
  `[mat-snack-bar]` s'il ne concerne aucun champ. Le code de règle (`R-TRX-10`…) n'est pas montré.
- **Confirmation** avant toute action destructrice ou irréversible (`[mat-dialog]`), avec le verbe de
  l'action sur le bouton (« Supprimer », jamais « OK »).
- **Annuler plutôt que confirmer** quand c'est possible : après une suppression simple, une snack-bar
  « Transaction supprimée — Annuler » pendant 5 secondes.
- **États vides** : chaque liste vide explique quoi faire (« Aucun compte. Créer un compte »). Aucun
  chiffre de démonstration (spec 08).

## Navigation

| Entrée | Écran | Phase 1 |
|---|---|---|
| Accueil | [E1](#e1--accueil-tdb-01) | ✓ |
| Comptes | [E2](#e2--comptes-cpt-01-cpt-03) | ✓ |
| Réglages | [E9](#e9--réglages) | ✓ |
| Transactions (tous comptes) | — | V1 (TRX-08) |
| Import, Rapprochement, Budget, Épargne, Projections | — | phases 2 à 6 |

**Mobile** : barre de navigation en bas de l'écran (3 entrées, icône et libellé) et bouton flottant
**« + »** au-dessus, à droite, qui ouvre la saisie rapide ([E5](#e5--saisie-rapide-trx-01-trx-05)) depuis
n'importe quel écran (TRX-01). Material n'a pas de composant « barre de navigation du bas » : elle est
construite avec `[mat-toolbar]` et des boutons `[routerLinkActive]`.

**Ordinateur** : menu latéral permanent `[mat-sidenav mode="side"]` avec les mêmes entrées, et un bouton
« Nouvelle transaction » en tête du menu.

```
Mobile                                Ordinateur
┌──────────────────────────┐          ┌──────────────┬──────────────────────────────────┐
│ Cashflow              ⋮  │          │ Cashflow     │                                  │
├──────────────────────────┤          │              │                                  │
│                          │          │ [+ Nouvelle  │                                  │
│        (écran)           │          │  transaction]│           (écran)                │
│                          │          │              │                                  │
│                     ╭───╮│          │ ▸ Accueil    │                                  │
│                     │ + ││          │   Comptes    │                                  │
│                     ╰───╯│          │   Réglages   │                                  │
├──────────────────────────┤          │              │                                  │
│ Accueil Comptes Réglages │          │ Déconnexion  │                                  │
└──────────────────────────┘          └──────────────┴──────────────────────────────────┘
```

## E0 — Connexion

Authentification mono-utilisateur (ADR-0015). Un seul champ **mot de passe** et le bouton
« Se connecter ». Pas d'identifiant : il n'y a qu'un utilisateur.

- Mot de passe incorrect : message sous le champ. Après plusieurs échecs, le délai d'attente imposé
  par le serveur est affiché (« Réessayer dans 30 s »).
- Aucun mot de passe défini : message expliquant de lancer `cashflow set-password` sur le serveur.
- Après connexion, retour à l'écran demandé, sinon à l'accueil.

## E1 — Accueil (TDB-01)

But : savoir en un coup d'œil où j'en suis, et traiter ce qui demande une action.

Blocs, de haut en bas :
1. **Patrimoine** : montant à aujourd'hui (R-TDB-1) et variation depuis la fin du mois précédent
   (R-TDB-2). Un toucher affiche le détail : actif brut et dettes.
2. **Mois en cours** : revenus, dépenses, épargne du mois et taux d'épargne (R-TDB-3 à R-TDB-7),
   marqués « mois en cours » (R-TDB-13), avec la comparaison au mois précédent à la même date.
   Chaque chiffre a une info-bulle « comment est-ce calculé ? » (spec 08).
3. **À catégoriser** : les ventilations non catégorisées (R-TDB-5), les plus récentes d'abord, 5 au
   maximum puis « Voir tout ». Chaque ligne permet de choisir la catégorie **sur place**
   `[mat-autocomplete]`, sans ouvrir la transaction.
4. **Comptes** : liste groupée par famille avec le solde de chaque compte, comme
   [E2](#e2--comptes-cpt-01-cpt-03) mais sans les comptes clôturés. Un toucher ouvre le compte.
5. **Alertes** : transactions `PENDING` de plus de 10 jours et écarts de rapprochement (R-TDB-10).
   **Bloc absent en phase 1** : toutes les transactions y sont saisies à la main et restent `PENDING`,
   l'alerte se déclencherait partout. Il apparaît en phase 2, avec le pointage par import.

Sur ordinateur, les blocs se répartissent sur deux colonnes : patrimoine et mois en cours à gauche,
comptes à droite, « à catégoriser » et alertes en dessous.

```
┌──────────────────────────┐
│ Patrimoine               │
│ 48 230,15 €   ▲ 612,40 € │
├──────────────────────────┤
│ Mars · mois en cours     │
│ Revenus      3 000,00 €  │
│ Dépenses     1 750,00 €  │
│ Épargne      1 250,00 €  │
│ Taux d'épargne   41,7 %  │
├──────────────────────────┤
│ À catégoriser (7)        │
│ 12 mars  CARREFOUR  -95,00│
│          [Catégorie…  ▾] │
│ 11 mars  SNCF       -42,00│
│          [Catégorie…  ▾] │
│ Voir tout                │
├──────────────────────────┤
│ Comptes                  │
│ Liquidités    2 140,12 € │
│  Compte courant 2 090,12 │
│  Espèces           50,00 │
│ Épargne fixe 16 090,03 € │
│  Livret A      12 000,00 │
│ …                        │
└──────────────────────────┘
```

## E2 — Comptes (CPT-01, CPT-03)

Liste de tous les comptes, **groupés par famille** (Liquidités, Épargne à taux fixe, Épargne variable,
Passifs), avec le total de chaque groupe. Ordre dans un groupe : `display_order`, puis nom.

- Chaque ligne : nom, établissement, solde (CPT-03). Pour un compte d'épargne variable ou un prêt, la
  valeur suit R-CPT-6 ; en phase 1, sans valorisation (CPT-06 en V1), c'est le solde d'ouverture plus
  les flux.
- Un compte en devise autre que l'euro porte la devise et la mention « hors totaux » (R-CPT-2).
- Les **comptes clôturés** sont regroupés dans une section repliée en bas `[mat-expansion-panel]`.
- Action : « Nouveau compte » ([E4](#e4--créer-ou-modifier-un-compte-cpt-01-cpt-04)).

## E3 — Détail d'un compte (CPT-03, TRX-02)

**En-tête** :
- nom, nature, établissement, IBAN masqué (R-CPT-8 ; un bouton l'affiche en clair) ;
- **solde total** et **solde pointé** (CPT-03). En phase 1, seul le solde total est affiché : sans
  pointage, le solde pointé ne voudrait rien dire. Il apparaît en phase 2 ;
- « Solde au… » : un sélecteur de date affiche le solde à une date passée ;
- menu `⋮` : Modifier, Clôturer (CPT-04), Supprimer (seulement si le compte est vide, R-CPT-7).

**Transactions** (TRX-02), les plus récentes en haut :
- colonnes : date, libellé, catégorie (ou « Ventilée » s'il y a plusieurs ventilations, ou « Virement
  vers / depuis <compte> »), montant, **solde après opération** ;
- `PENDING` en italique avec une pastille « en attente » ; `RECONCILED` avec un cadenas. Comme le solde
  pointé, ces marques n'apparaissent qu'en phase 2 ;
- **filtres** : période, catégorie, statut, montant (min, max), texte (libellé et note). Sur mobile, ils
  sont dans un panneau qui s'ouvre depuis une icône ; les filtres actifs apparaissent en puces
  `[mat-chip]` sous l'en-tête ;
- sur ordinateur : `[mat-table]` triable avec défilement virtuel `[cdk-virtual-scroll]` ; sur mobile :
  une liste groupée par jour, montant à droite, solde après opération en petit dessous ;
- un toucher ouvre la transaction ([E7](#e7--détail-dune-transaction-trx-01-trx-03-trx-05)) ;
- le bouton « + » ouvre la saisie rapide avec ce compte présélectionné.

```
┌──────────────────────────┐
│ ← Compte courant      ⋮  │
│ LCL · FR76 •••• •••4 21  │
│ Solde       2 090,12 €   │
│ Pointé      2 135,12 €   │  ← phase 2
│ [Solde au… 📅]           │
├──────────────────────────┤
│ 🔍 Filtres  [Mars ×]     │
├──────────────────────────┤
│ Mercredi 12 mars         │
│ CARREFOUR MARKET  -120,00│
│ Ventilée        2 090,12 │
│ Mardi 11 mars            │
│ Virement → Livret A      │
│                  -200,00 │
│                 2 210,12 │
│ ┆ en attente ┆ ← phase 2 │
└──────────────────────────┘
```

## E4 — Créer ou modifier un compte (CPT-01, CPT-04)

Formulaire `[mat-dialog]` sur ordinateur, plein écran sur mobile.

1. **Nature** (requis) : liste groupée par famille. Elle n'est plus modifiable dès que le compte a des
   transactions (R-CPT-1).
2. **Nom** (requis), **établissement** (optionnel, liste des établissements non archivés, R-CPT-10, avec
   « Nouvel établissement… » en bas de liste).
3. **Date d'ouverture** et **solde d'ouverture** (requis, défaut 0). Pour un prêt, le capital restant dû
   est saisi en positif et enregistré en négatif (R-CPT-12), ce que le libellé du champ indique.
4. **Coordonnées bancaires**, section repliée : IBAN (validé à la saisie, R-CPT-8), numéro de compte.
5. **Options**, section repliée, préremplies selon la nature : compter dans le patrimoine, dans
   l'épargne de précaution (R-CPT-13), dans les budgets (R-CPT-11). Devise (EUR par défaut, non
   modifiable après création). Note.

Compte d'épargne à taux fixe (Livret A, LDDS…) : ses conditions d'épargne (R-CPT-9) sont créées
**préremplies** selon la nature, sans être affichées en phase 1. Leur écran (CPT-05) arrive avec
l'épargne (phase 5), qui les exploite pour calculer les intérêts.

**Clôturer** (CPT-04) : dialogue qui demande la date de clôture. Si le solde à cette date n'est pas nul,
il l'affiche et demande une confirmation explicite (R-CPT-5).

## E5 — Saisie rapide (TRX-01, TRX-05)

Accessible partout par le bouton « + ». Panneau qui monte du bas sur mobile `[mat-bottom-sheet]`,
dialogue sur ordinateur. Pensée pour être remplie en quelques secondes au téléphone.

En tête, trois modes `[mat-button-toggle]` : **Dépense** (par défaut), **Revenu**, **Virement**.

Dépense ou revenu :
- **montant** en premier, avec le focus et le clavier numérique (`inputmode="decimal"`) ;
- **compte** (le dernier utilisé, ou celui de l'écran courant) ;
- **catégorie** `[mat-autocomplete]` : recherche dans « parent › sous-catégorie », catégories de la
  nature du mode d'abord (dépenses pour une dépense) ;
- **libellé**, **date** (aujourd'hui par défaut), **note** (repliée) ;
- lien **« Ventiler »** : ouvre l'éditeur de ventilation ([E6](#e6--ventilation-trx-03)) ;
- bouton « Enregistrer ». La transaction est créée `PENDING`, origine `MANUAL` (TRX-01). Une snack-bar
  confirme, avec « Voir ».

Virement (TRX-05, R-TRX-33) :
- montant, **compte de départ**, **compte d'arrivée** (différent, même devise, R-TRX-30), date, libellé ;
- pas de catégorie (R-TRX-32). Les deux jambes sont créées d'un coup.

```
┌──────────────────────────┐
│ [Dépense|Revenu|Virement]│
│                          │
│        95,00 €           │
│                          │
│ Compte   Compte courant ▾│
│ Catégorie  Alim › Super ▾│
│ Libellé  Carrefour       │
│ Date     12/03/2026   📅 │
│ Ventiler   Note          │
│                          │
│ [     Enregistrer      ] │
└──────────────────────────┘
```

## E6 — Ventilation (TRX-03)

Ouvert depuis la saisie rapide ou depuis le détail d'une transaction.

- En haut : le montant de la transaction et le **reste à ventiler**, en rouge tant qu'il n'est pas nul.
- Une ligne par ventilation : catégorie, montant **ou** pourcentage (bascule globale « € / % »), note,
  bouton de suppression. « Ajouter une ligne » en bas.
- En pourcentages, les montants calculés s'affichent à côté de chaque ligne ; l'arrondi va sur la
  dernière ligne (R-TRX-14).
- Bouton « Répartir le reste » sur une ligne : lui attribue le reste à ventiler.
- Une ligne de signe opposé à la transaction est signalée (remboursement, R-TRX-11).
- « Valider » n'est actif que si le reste vaut 0,00 € (R-TRX-10).
- Le rattachement à un projet (R-TRX-15) arrive avec les budgets (phase 4).

## E7 — Détail d'une transaction (TRX-01, TRX-03, TRX-05)

Plein écran sur mobile, panneau latéral sur ordinateur (la liste reste visible à gauche).

- Champs modifiables : montant, date, libellé, note, catégorie (ou ventilation via E6). Le libellé
  bancaire, s'il existe, est affiché en lecture seule sous le libellé (R-TRX-2).
- Informations : statut, origine, date bancaire et date de valeur si elles existent.
- Une transaction ventilée affiche ses ventilations ; modifier le montant demande de rééquilibrer la
  ventilation (R-TRX-4).
- Une transaction `RECONCILED` affiche les champs verrouillés avec un cadenas et l'explication
  (R-TRX-3). Le déverrouillage arrive avec le rapprochement (phase 2).
- **Virement** : la jambe affiche le compte de contrepartie, avec un lien vers l'autre jambe, et
  l'action « Délier » (R-TRX-34). Une transaction ordinaire propose « Marquer comme virement » :
  choix du compte de contrepartie, puis des transactions candidates s'il y en a, sinon création de la
  jambe opposée (R-TRX-35, R-TRX-36).
- Menu `⋮` : Supprimer (avec « Annuler » dans la snack-bar), Dupliquer.

## E8 — Catégories (TRX-04)

Dans les réglages. Deux onglets `[mat-tab-group]` : **Dépenses** et **Revenus**.

- Arbre à deux niveaux (R-TRX-20) : catégorie, sous-catégories en retrait. Ordre modifiable par
  glisser-déposer `[cdkDrag]`.
- Actions par catégorie : Renommer, Ajouter une sous-catégorie, **Archiver** (masquée en saisie,
  conservée dans l'historique), **Fusionner dans…** (réaffecte toutes les ventilations après
  confirmation, en indiquant leur nombre), Supprimer si elle n'a jamais servi (R-TRX-22).
- Les catégories archivées sont regroupées en bas, repliées, avec « Désarchiver ».

## E9 — Réglages

Liste d'entrées, chacune ouvrant sa page :

- **Catégories** ([E8](#e8--catégories-trx-04)).
- **Établissements** (CPT-02) : liste préremplie (données initiales), ajout, modification (nom, code
  banque, BIC), archivage (R-CPT-10).
- **Export** (ADR-0008) : bouton « Télécharger l'export complet » (JSON). La case « inclure les fichiers
  source » arrivera avec l'import (phase 2).
- **Sécurité** : « Se déconnecter de tous les appareils » (ADR-0015), avec confirmation.
- **À propos** : version de l'application, date de la dernière sauvegarde automatique.
