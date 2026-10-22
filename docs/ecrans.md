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
- **Largeur** : chaque écran prend toute la largeur disponible, avec les mêmes marges à gauche et à
  droite (1rem sur mobile, 2rem sur ordinateur).
- **Montants** : format français (`1 234,56 €`), alignés à droite dans les tableaux, en rouge pour une
  sortie, sans signe `+` pour une entrée. Saisie toujours positive, le sens est choisi à part
  (dépense ou revenu).
- **Champs** : tous de la même taille, compacts (densité -3 des champs Material, 44 px de haut), y
  compris les montants, et séparés partout du même écart (1rem, variable CSS `--field-gap`) ; texte
  des champs à 16 px (sous cette taille, Safari sur iPhone zoome à la saisie).
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

Sur ordinateur, les blocs se répartissent sur autant de colonnes que
la place le permet (une colonne d'au moins 22rem, quatre au plus). En deux colonnes : patrimoine et mois
en cours à gauche, comptes à droite, « à catégoriser » et alertes en dessous.

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


**Dernières saisies.** Encadré des 5, 10 ou 15 dernières transactions **saisies** (ordre de
création, quelle que soit leur date d'opération) : date, compte, libellé (en italique si en attente),
catégorie, ventilation ou virement, montant ; un clic ouvre la transaction (E7). Le nombre affiché se
choisit dans le menu de réglages de l'encadré.

**Réglages d'un encadré.** Chaque encadré porte, à côté de sa poignée, un bouton de réglages discret
qui ouvre un menu : les options d'affichage propres à l'encadré, puis « Masquer l'encadré ». Options :

| Encadré | Options (valeur par défaut) |
|---|---|
| Patrimoine | Détail actif brut / dettes (non ; un clic sur le montant bascule aussi) |
| Mois en cours | Comparer au mois précédent (oui) |
| Dernières saisies | 5, 10 ou 15 transactions (5) |
| Budget du mois | Objectifs d'épargne (oui), Projets (oui) |
| Épargne | Épargne de précaution, Intérêts de l'année, Épargne variable (oui) |
| Comptes | Détail des comptes (oui ; sinon les familles seules) |

Les encadrés masqués se réaffichent depuis le bouton « Encadrés masqués (n) » en haut de l'accueil.
Réglages et encadrés masqués sont gardés dans le navigateur (`localStorage`), pour tous les nombres de
colonnes. Tous les encadrés ont le même style de titre ; un titre qui mène à un écran (Budget, Comptes,
Échéances) n'est souligné qu'au survol.

**Disposition personnalisable.** Chaque encadré porte une poignée (⋮⋮, en haut à droite) : on le fait
glisser pour le réordonner dans sa colonne, ou le changer de colonne. La disposition est gardée dans le
navigateur (`localStorage`), séparément pour chaque nombre de colonnes (une sur mobile, jusqu'à quatre sur
grand écran) ; « Disposition par défaut » apparaît dès que celle du nombre de colonnes affiché a été
modifiée. Un encadré sans contenu (aucune alerte, rien à catégoriser) reste masqué à sa place.

## E2 — Comptes (CPT-01, CPT-03)

Liste de tous les comptes, **groupés par famille** (Liquidités, Épargne à taux fixe, Épargne variable,
Biens, Passifs), avec le total de chaque groupe. Ordre dans un groupe : `display_order`, puis nom.

- Chaque ligne : nom, établissement, solde (CPT-03). Pour un compte d'épargne variable, un bien ou un
  prêt, la valeur suit R-CPT-6 ; en phase 1, sans valorisation (CPT-06 en V1), c'est le solde d'ouverture plus
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
   est saisi en positif et enregistré en négatif (R-CPT-12), ce que le libellé du champ indique. Pour un
   bien, le champ s'intitule « Valeur du bien » et suggère le prix d'achat (R-CPT-14).
4. **Coordonnées bancaires**, section repliée : IBAN (validé à la saisie, R-CPT-8), numéro de compte.
5. **Options**, section repliée, préremplies selon la nature : compter dans le patrimoine, dans
   l'épargne de précaution (R-CPT-13), dans les budgets (R-CPT-11). Devise (EUR par défaut, non
   modifiable après création). Note.

Pour un prêt, les **conditions** (R-CPT-15) se saisissent ensuite depuis sa fiche
([E3](#e3--fiche-dun-prêt--tableau-damortissement-cpt-09-cpt-10)), dans une popup : capital emprunté,
taux nominal, durée en mois, date de la première échéance, assurance (aucune, montant mensuel ou taux
annuel sur le capital), compte payeur, catégories des intérêts et de l'assurance (« Prêts › Intérêts »,
« Prêts › Assurance emprunteur » par défaut). La mensualité calculée s'affiche pendant la saisie.

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
- **Mensualité de prêt** (R-CPT-17) : un débit qui correspond à une échéance non payée affiche « Correspond à
  l'échéance n° N du prêt X : capital, intérêts, assurance » et « Ventiler selon l'échéance ». Une mensualité
  ventilée affiche « Échéance n° N du prêt X » (lien vers le prêt), sa répartition (« Capital remboursé »,
  intérêts, assurance) et « Annuler la ventilation » ; son montant est verrouillé. La jambe du capital, sur le
  prêt, renvoie à la même échéance.
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

---

# Écrans — phases 2 et 3

Statut : **Validée** (2026-10-21)

Ce qui change dans les écrans de la phase 1 :
- **E1 Accueil** : le bloc **Alertes** apparaît (écart de rapprochement, transactions en attente depuis
  plus de 10 jours, R-TDB-10), avec un lien vers le compte concerné.
- **E3 Détail d'un compte** : le **solde pointé** et les marques de statut apparaissent (« en attente »
  en italique, cadenas pour une transaction rapprochée) ; filtre par statut ; case de pointage manuel
  sur chaque transaction en attente (RAP-07) ; panneau **Rapprochement** (E13) ; action « Importer un
  relevé » dans le menu.
- **E7 Détail d'une transaction** : statut affiché ; « Pointer » / « Dépointer » ; « Créer une règle à
  partir de celle-ci » (R-IMP-20).

## Navigation

| Entrée | Mobile (barre du bas) | Ordinateur (menu latéral) |
|---|---|---|
| Accueil, Comptes | ✓ | ✓ |
| Transactions (E14) | ✓ | ✓ |
| Analyses (E15) | ✓ | ✓ |
| Import (E10, E12) | depuis un compte ou les réglages | ✓ |
| Réglages | ✓ | ✓ |

## E10 — Import : assistant (IMP-01, IMP-02)

Conçu pour l'ordinateur, utilisable sur mobile. Étapes `[mat-stepper]` :
1. **Fichier** : dépôt du fichier, compte, profil existant ou « nouveau profil ». Si le même fichier a
   déjà été validé dans ce compte, un avertissement (R-IMP-7), non bloquant.
2. **Colonnes** (nouveau profil, CSV/XLSX) : encodage et séparateur détectés et modifiables, feuille
   (XLSX), ligne des en-têtes, lignes à ignorer ; association des colonnes (date et format, libellé(s),
   montant unique ou débit/crédit, séparateurs) ; filtres de lignes ; cellules des soldes. Les 20
   premières lignes brutes sont affichées, et **l'aperçu normalisé se met à jour en direct** avec les
   erreurs par ligne. L'OFX saute cette étape.
3. **Profil** : nom du profil à enregistrer (facultatif), puis « Lancer la revue ».

## E11 — Import : revue (IMP-04, IMP-05, R-IMP-18)

- En-tête : compte, fichier, période ; **contrôle du relevé** (`fin − début − Σ lignes`) en vert si nul,
  en rouge sinon (R-IMP-18) ; compteurs à créer / à pointer / ignorées / en erreur.
- Tableau des lignes : date, libellé bancaire, montant, **décision** (Créer / Pointer / Ignorer), et selon
  la décision : catégorie (ou ventilation) et libellé de la transaction à créer, ou la transaction
  pointée avec son score ; doublons grisés ; lignes en erreur avec leurs erreurs, non importables.
- Proposition de virement (R-IMP-14) : « Virement vers Livret A ? Accepter ».
- Filtres : toutes, à créer, à pointer, doublons, erreurs, non catégorisées. Sélection multiple : décision
  ou catégorie en masse.
- Pied : « Annuler l'import » (supprime le lot, R-IMP-1) et « Valider » ; après validation, résumé et,
  pour chaque point de contrôle à écart nul, **« Rapprocher au <date> »** (R-RAP-11), et, si des mensualités
  de prêt sont reconnues parmi les transactions du lot, **« Ventiler selon les échéances »** (R-IMP-22).

## E12 — Import : historique (IMP-06, IMP-07)

Liste des lots : date, compte, fichier, période, statut, nombre de transactions créées et pointées.
Actions : reprendre la revue (lot en revue), **annuler un lot validé** (R-IMP-17, avec confirmation),
télécharger ou supprimer le fichier source (R-IMP-21).

## E13 — Rapprochement d'un compte (RAP-01 à RAP-06)

Panneau de la fiche compte (E3), pour les comptes de liquidités et d'épargne à taux fixe (R-RAP-10) :
- liste des points de contrôle : date, solde annoncé, solde calculé, **écart**, état (rapproché ✓,
  écart figé, à rapprocher) ;
- « Ajouter un point de contrôle » : date et solde lu sur la banque ;
- sur un point à écart nul : « Rapprocher » → dialogue qui liste les transactions encore en attente et les
  points dont l'écart sera figé (R-RAP-4, R-RAP-5) ;
- sur un point en écart : « Chercher l'écart » → pistes (R-RAP-9) : transactions en attente, montant
  égal à l'écart, paire de transactions, erreur de signe ;
- sur un point rapproché : « Déverrouiller » avec confirmation (R-RAP-6).

## E14 — Transactions (TRX-08, TRX-06)

Toutes les transactions, tous comptes confondus, avec les filtres d'E3 plus le compte et « à
catégoriser ». Sélection multiple (cases à cocher) et barre d'action « Catégoriser » (TRX-06). Pas de
solde après opération : il n'a de sens que par compte.

## E15 — Analyses (TDB-02, TDB-03)

Deux onglets :
- **Par catégorie** : mois choisi (par défaut le mois en cours, « mois en cours »), dépenses ou revenus ;
  graphique en barres horizontales par catégorie, et tableau : montant, mois précédent (à la même date
  pour le mois en cours, R-TDB-13), moyenne 12 mois ; une catégorie se déplie en sous-catégories ; un
  clic ouvre E14 filtré sur la catégorie et le mois.
- **Évolution** : 12 ou 24 mois ; barres revenus / dépenses et courbe d'épargne ; taux d'épargne du mois
  et glissant sur 12 mois.

Graphiques : ECharts (ADR-0009).

## E16 — Règles de catégorisation (IMP-08)

Dans les réglages. Liste par priorité (glisser-déposer), avec nombre d'utilisations, actif / inactif.
Formulaire : conditions (libellé contient / commence par / égal / motif avec `*`, montant, sens,
comptes), actions (libellé, catégorie ou ventilation en %, virement vers un compte, note),
« arrêter ici ». « Aperçu » liste les transactions existantes non catégorisées concernées, qu'on peut
cocher et catégoriser d'un coup (R-IMP-19).

## E17 — Profils d'import (IMP-02)

Dans les réglages : liste des profils (nom, format, compte par défaut), renommer, supprimer, modifier
les réglages (mêmes champs que l'étape 2 de l'assistant).

# Écrans — phase 4 (budgets)

Statut : **Validée** (2026-10-21)

Ce qui change dans les écrans existants :
- **Navigation** : entrée **Budget** (E18). Sur mobile, la barre du bas garde cinq entrées (Accueil,
  Comptes, Transactions, Budget, Analyses) ; **Réglages**, Import et Déconnexion passent dans le menu ⋮
  de la barre du haut.
- **Cloche de notifications** (E21) en haut de l'écran sur mobile, à côté du nom de l'application sur
  ordinateur, avec un badge du nombre de non lues.
- **E1 Accueil** : bloc **Budget du mois** (TDB-06) dans la colonne de droite : reste à budgéter, lignes
  en alerte avec leur jauge, objectifs d'épargne du mois, projets en cours. Absent tant qu'aucun budget
  ni projet n'existe.
- **E7 Détail d'une transaction** : champ **Projet** (en cours, ou le projet actuel même terminé). Le
  projet s'applique à toutes les ventilations de la transaction.
- **E14 Transactions** : dans la barre d'action de la sélection, **Projet** puis « Rattacher » (ou
  « Aucun » pour détacher).
- **E15 Analyses** : case « Inclure les projets » (TDB-07).
- **E16 Règles** : action « Rattacher au projet ».

## E18 — Budget du mois (BUD-02 à BUD-05, BUD-07)

- Mois affiché (flèches, « mois en cours »), mémorisé dans l'adresse (`?mois=AAAA-MM`).
- Deux chiffres en tête : **reste à budgéter** (prévu, R-BUD-40) et **solde du mois** (réel, R-BUD-41),
  avec leur définition en infobulle.
- Sections **Revenus**, **Dépenses mensuelles**, **Dépenses annuelles** (quote-part du mois et
  consommé sur l'année), **Épargne** : prévu, réel, reste, jauge colorée selon les seuils (R-BUD-61),
  pastille du report, icône d'une période surchargée.
- **Hors lignes** : dépenses hors budget par catégorie, non catégorisé, projets exclus ; un clic ouvre
  E14 filtré sur le mois.
- Un clic sur une ligne ouvre sa modification : montant **à partir de ce mois** ou **pour ce mois
  seulement** (pour cette année budgétaire, ligne annuelle), avec un avertissement sur un mois passé
  (R-BUD-52) ; report ; alertes aux seuils habituels, à des seuils propres ou **pas d'alerte** (budget
  fixe, signalé par une icône sur la ligne, R-BUD-60) ; « Remettre le report à zéro » ; « Revenir au montant
  habituel » ; « Supprimer à partir de ce mois » (R-BUD-53).
- « Ajouter une ligne » : dépense, revenu ou épargne ; catégorie ou compte d'épargne ; par mois ou par
  an (mois de début de l'année budgétaire) ; montant ; report ; « Pas d'alerte ».
- Menu ⋮ : assistant (E19), seuils d'alerte globaux.
- Sans aucune ligne : invitation à lancer l'assistant.

## E19 — Assistant de création (BUD-10)

Une ligne par catégorie qui a des mouvements sur les 6 derniers mois complets (moins s'il y a moins
d'historique) : montants mois par mois, proposition (moyenne arrondie à l'euro supérieur) modifiable,
case à cocher. Les catégories qui ont déjà une ligne sont grisées. « Créer le budget » crée les lignes
cochées à partir du mois en cours.

## E20 — Projets (BUD-06)

- **Liste** : en cours, puis terminés ; réel sur prévu et jauge. « Nouveau projet ».
- **Détail** : réel et jauge, dates, exclusion des lignes du mois ; réel **par catégorie** ; transactions
  rattachées, celles hors des dates signalées (R-BUD-23). « Modifier » (y compris clôturer) ;
  « Supprimer » seulement sans transaction rattachée.

## E21 — Notifications (BUD-08)

Menu de la cloche : notifications les plus récentes (titre, détail, date), non lues en gras ; un clic
marque lue et ouvre le budget du mois ou le projet ; « Tout marquer comme lu ». Le serveur évalue les
seuils après chaque écriture et regroupe en une notification les seuils franchis ensemble (R-BUD-62,
R-BUD-63).


# Écrans — phase 5 (épargne)

Statut : **Validée** (2026-10-21)

Ce qui change dans les écrans existants :
- **E1 Accueil** : bloc **Épargne** (TDB-05) sous le budget : épargne de précaution en mois de dépenses,
  intérêts de l'année (courus et attendus) par livret, valeur, capital investi et TRI global de l'épargne
  variable, puis par compte. Un livret dont les intérêts de l'an dernier restent à inscrire est signalé.
- **E9 Réglages** : entrée **Barèmes de taux** (E22).

## E3 — Fiche d'un compte à taux déterminé : panneau Intérêts (EPA-02 à EPA-04, CPT-05)

- Barème et taux en vigueur ; année affichée (flèches), gardée quand la fiche se recharge.
- Année en cours : intérêts **courus** depuis le 1er janvier (quinzaines terminées) et **attendus** au
  31 décembre sans nouveau mouvement. Année passée : calculés, **inscrits** et l'écart éventuel.
- Capital versé sur plafond, et un avertissement daté si un versement l'a dépassé (R-EPA-9).
- Année finie sans transaction d'intérêts : « Inscrire les intérêts AAAA » crée une transaction en
  attente au 31/12, que l'import du relevé pointera (R-EPA-10).
- « Détail par quinzaine » : solde en valeur, taux, intérêt ; les quinzaines à venir en italique.
- « Conditions » : barème (lien vers E22), méthode (seules les quinzaines sont calculées en V1),
  plafond, catégorie des intérêts, arrondi à chaque quinzaine (R-EPA-6).

## E3 — Fiche d'un compte à rendement variable, d'un bien ou d'un prêt (CPT-06, EPA-05, EPA-06)

- **Performance** (épargne variable) : valeur (« estimée » si la dernière valorisation a plus de
  90 jours), capital investi, plus-value en € et en %, TRI depuis l'ouverture et depuis le 1er janvier,
  performance pondérée par le temps (signalée comme une approximation), tableau par année civile
  (« données insuffisantes » sans valorisation à ±15 jours du 31/12).
- **Valorisations** (ou **Capital restant dû** pour un prêt, saisi en positif et enregistré en
  négatif ; **Valeur du bien** pour un bien, sans bloc Performance) : liste datée, « Ajouter », suppression ; une saisie à une date déjà valorisée demande de
  confirmer le remplacement.

## E3 — Fiche d'un prêt : tableau d'amortissement (CPT-09, CPT-10)

Pour un prêt qui a des conditions (R-CPT-15) :

- **Résumé** : mensualité (hors assurance et totale), capital restant dû selon le compte et selon le
  tableau, avec l'écart s'il y en a un ; coût total du crédit (intérêts + assurance) ; date de fin.
- **Tableau d'amortissement**, une année à la fois (l'année de la prochaine échéance par défaut) : n°,
  date, mensualité, capital, intérêts, assurance, capital restant dû ; les échéances payées sont
  marquées et mènent à leur transaction ; la prochaine est mise en évidence. Sur mobile : n°, date,
  mensualité et capital restant dû.
- Actions : « Conditions » (popup de saisie) et retrait des conditions, après confirmation : le prêt
  revient au suivi par valorisations.
- Action « Ventiler les mensualités passées », affichée quand des mensualités déjà enregistrées correspondent à
  des échéances non payées (R-CPT-17) : liste à cocher (tout coché par défaut), puis validation. Une échéance
  payée porte une coche, avec « Voir la mensualité » et « Annuler la ventilation ».

Sans conditions, un bouton « Saisir les conditions du prêt » ouvre la popup de saisie.

## E22 — Barèmes de taux (EPA-01)

Dans les réglages. Un bloc par barème (réglementé ou propre à un compte) : comptes qui l'utilisent,
taux successifs datés (le plus récent en premier, supprimables), ajout d'un taux à une date d'effet. Un
rappel invite à vérifier les taux réglementés à la source officielle (R-EPA-12).

# Écrans — phase 6 (récurrences et projections)

Statut : **Validée** (2026-10-21)

Ce qui change dans les écrans existants :
- **Navigation** : entrée **Projections** (E24) dans le menu de l'ordinateur ; sur mobile, Projections et
  **Récurrences** (E23) sont dans le menu ⋮ du haut.
- **E1 Accueil** : bloc **Échéances non vues** (R-TRX-42) quand une échéance passée n'a été honorée par
  aucune transaction.
- **E9 Réglages** : entrée **Récurrences**.

## E23 — Récurrences (TRX-07)

Deux onglets :
- **Échéancier** : échéances des 60 derniers jours et des 60 prochains, avec leur compte (et le compte
  d'arrivée d'un virement) et leur état : honorée (lien vers la transaction), **non vue** en rouge, à
  venir. « Créer » matérialise une échéance en transaction en attente avec la catégorie de la
  récurrence (R-TRX-43).
- **Récurrences** : liste (rythme, compte, prochaine échéance, nombre d'échéances non vues) ; un clic
  ouvre la modification.

Formulaire : dépense, revenu ou virement (compte d'arrivée) ; libellé, compte, catégorie, montant, écart
toléré, rythme (« tous les N semaine / mois / trimestre / an »), première échéance, fin, active.

## E24 — Projections (PRJ-01 à PRJ-05, TDB-04)

Quatre onglets, chacun avec ses hypothèses affichées et le rappel qu'il s'agit d'hypothèses, non de
prévisions (R-PRJ-3, R-PRJ-31) :
- **Patrimoine** : courbe réelle des 24 derniers mois (TDB-04), prolongée en pointillés par chaque
  scénario ; horizon 5, 10, 20 ou 30 ans ; valeur finale par scénario (et en euros constants si le
  scénario a une inflation) ; comptes partis d'une valorisation ancienne signalés.
- **Trésorerie** : compte de liquidités, 30, 90 ou 180 jours, case « Dépenses courantes moyennes »
  (R-PRJ-11) ; solde du jour, solde le plus bas et sa date (avertissement sous zéro, R-PRJ-12), courbe
  en escalier, liste des mouvements connus (échéances prévues et transactions en attente).
- **Épargne** : compte d'épargne, scénario, horizon ; valeur du jour et à l'horizon, courbe, hypothèses
  (taux et plafond pour un livret, rendement et frais pour l'épargne variable, versement mensuel).
- **Scénarios** : un bloc par scénario (horizon, inflation, rendements par nature) ; « Modifier » ouvre
  les hypothèses : rendement et frais par nature, réglages par compte (dont le versement mensuel), taux
  futurs des barèmes (R-PRJ-21) ; scénario par défaut ; suppression sauf pour le scénario par défaut.
