# Spec 13 — Simulations et demandes de crédit

Statut : **Validée** (2026-10-29)
Modèle : [LoanSimulation, LoanApplication, LoanApplicationPiece, LenderRequest](../modele-de-donnees.md#crédit--spec-13)
Décision : [ADR-0027](../adr/0027-simulations-et-demandes-de-credit.md)

## Concepts

La rubrique **Prêts** (spec 09) suit les prêts en cours. Elle aide aussi à préparer les prochains :

- une **simulation** (`LoanSimulation`) calcule ce que coûterait un crédit : mensualité, coût total, TAEG,
  tableau d'amortissement. Elle est enregistrée : on la compare à d'autres, on la rattache à une demande, et
  une fois l'offre obtenue on la **transforme en prêt** ;
- une **demande de crédit** (`LoanApplication`) est le dossier d'un projet à financer : le **plan de
  financement** (prix, frais, apport, montant à emprunter), la **capacité d'emprunt** calculée à partir de mes
  données (salaires, prêts en cours, comptes) et d'un éventuel co-emprunteur, la liste des **pièces à
  fournir**, reliée à la bibliothèque de documents (spec 12), et le **suivi des démarches** auprès des banques
  et courtiers, avec les offres reçues et leur comparaison.

```
Demande « Achat appartement Grenoble »
 ├─ Plan de financement   prix 280 000 € + notaire 21 000 € + travaux 10 000 € − apport 41 000 € = 270 000 €
 ├─ Capacité              revenus 5 600 €/mois, charges 350 €, endettement 30,6 % (≤ 35 %)
 ├─ Simulations           « 25 ans à 3,4 % », « 20 ans à 3,2 % »
 ├─ Pièces à fournir      identité ✓, 3 fiches de paie ✓, 2 avis d'imposition 1/2, compromis ✗…
 └─ Démarches             LCL : offre reçue (simulation « Offre LCL ») ; Crédit Agricole : envoyée
```

- la **capacité d'achat** répond à la question inverse de la demande : non pas « ce projet passe-t-il ? », mais
  « **que puis-je acheter ?** ». À partir de mes salaires, de mes prêts, de mon épargne et de mes dépenses, elle
  calcule le prix d'achat maximal, aujourd'hui et dans les années à venir, et l'effet de l'achat sur mon
  épargne et mon patrimoine projeté.

Le **parcours** d'un achat :

1. je calcule ma **capacité d'achat**, puis j'optimise le crédit avec le **simulateur**, qui part des mêmes
   hypothèses (onglet **Simuler**) ;
2. d'une simulation, je **prépare une demande** (R-CRE-36) ;
3. j'envoie la demande à une ou plusieurs banques : chaque envoi est une **démarche**, que je suis et que je
   **relance** (R-CRE-24 à R-CRE-26) ;
4. je fournis les **pièces** demandées (R-CRE-17) ;
5. quand une banque répond favorablement, j'**accepte son offre** : l'application crée le **bien** et le
   **prêt**, et y range les documents du dossier (R-CRE-27).

Rien de tout cela n'entre dans le patrimoine, les budgets ou les projections : seuls le bien et le prêt créés à
l'acceptation comptent, comme tout bien ou prêt (spec 09).

## Simulations

- **R-CRE-1** — Une **simulation** a un nom (≤ 80 caractères), un type de prêt (R-PRE-1), un **capital**
  positif, un taux nominal annuel, une durée en mois (1 à 600), une assurance emprunteur (comme R-PRE-2 :
  aucune, montant mensuel fixe, ou taux annuel sur le capital), des **frais de dossier** et des **frais de
  garantie** (positifs ou nuls, payés une fois), et une note. Elle peut être rattachée à une demande de crédit
  (R-CRE-10) et à l'établissement qui la propose.
- **R-CRE-2** — Sa mensualité, son assurance et son tableau d'amortissement suivent R-PRE-3, la première
  échéance étant fixée, pour le calcul, un mois après la date du jour.
- **R-CRE-3** — **Coût total du crédit** = intérêts + assurance sur toute la durée + frais de dossier + frais
  de garantie.
- **R-CRE-4** — **TAEG** (taux annuel effectif global) : le taux actuariel annuel `T` tel que le capital,
  diminué des frais, égale la somme des échéances (mensualité + assurance) actualisées :
  `capital − frais = Σ (échéance k) / (1 + i)^k`, avec `i` le taux mensuel et `T = (1 + i)^12 − 1`. Il se
  calcule par dichotomie, en décimal (ADR-0001), au millionième près, et s'affiche avec deux décimales. Sans
  frais ni assurance, il est le taux nominal rendu actuariel.
- **R-CRE-5** — **Calculs inverses** : à partir d'une **mensualité visée** (assurance comprise), d'un taux,
  d'une durée et d'une assurance, le **capital empruntable** (arrondi à l'euro inférieur) ; à partir d'un
  capital, d'un taux, d'une assurance et d'une mensualité visée, la **durée** la plus courte en mois dont la
  mensualité ne dépasse pas la mensualité visée. Une mensualité visée qui ne couvre pas l'assurance, ou
  qu'aucune durée de 600 mois ou moins ne permet, est refusée.
- **R-CRE-6** — Plusieurs simulations se **comparent** côte à côte : mensualité, assurance, coût total,
  TAEG, durée, capital, et l'écart de chacune à la moins chère.
- **R-CRE-7** — **Transformer en prêt** : une simulation crée un prêt avec tableau (R-PRE-1, R-PRE-2) de mêmes
  type, capital, taux, durée et assurance ; on saisit la date de déblocage, la date de première échéance, le
  compte payeur et, s'il y a lieu, le bien financé. La simulation garde le lien vers le prêt créé ; supprimer
  le prêt retire ce lien. Une simulation déjà transformée ne l'est pas une seconde fois.

## Demandes de crédit

### Plan de financement

- **R-CRE-10** — Une **demande de crédit** a un nom, un **type de projet** (`EXISTING_HOME` achat dans
  l'ancien, `NEW_HOME` achat dans le neuf, `WORKS` travaux, `VEHICLE` véhicule, `OTHER` autre), un
  **statut** (R-CRE-20), éventuellement le **bien** concerné (R-PRE-10), et une note.
- **R-CRE-11** — Le **plan de financement** : prix (du bien ou de l'achat), **frais de notaire**, travaux,
  autres frais (agence, dossier, garantie…), **apport**. Le **coût du projet** est la somme du prix, des frais
  de notaire, des travaux et des autres frais ; le **montant à emprunter** est le coût du projet moins
  l'apport. L'apport ne dépasse pas le coût du projet.
- **R-CRE-12** — Les frais de notaire sont **estimés** à 7,5 % du prix dans l'ancien, 2,5 % dans le neuf, 0
  sinon, arrondis à l'euro ; l'estimation est modifiable. Ce sont des ordres de grandeur, signalés comme tels.

### Capacité d'emprunt

- **R-CRE-13** — **Revenus mensuels** de la demande, à la date du jour :
  - les nets mensuels en vigueur des emplois en cours (R-SAL-3), plus les primes récurrentes de la dernière
    année ramenées au mois (somme / 12) ;
  - les **autres revenus** mensuels saisis dans la demande (revenus locatifs, pensions…) ;
  - les revenus mensuels du **co-emprunteur**, saisis (R-CRE-16).
- **R-CRE-14** — **Charges mensuelles** de la demande :
  - les échéances totales (mensualité + assurance) des prêts en cours avec tableau, non soldés, dont il reste
    des échéances ; un prêt en suivi simple n'a pas de mensualité connue : on la compte dans les autres
    charges ;
  - les **autres charges** mensuelles saisies (loyer conservé, pension versée, prêt en suivi simple…) ;
  - les charges mensuelles du co-emprunteur, saisies.
  Les prêts que le nouveau crédit remboursera peuvent être **exclus** un par un.
- **R-CRE-15** — Avec la **mensualité envisagée** (celle de la simulation retenue, assurance comprise) :
  - **taux d'endettement** = (charges + mensualité envisagée) / revenus ;
  - **reste à vivre** = revenus − charges − mensualité envisagée ;
  - **mensualité maximale** = 35 % des revenus − charges, et le **capital empruntable** correspondant aux
    taux, durée et assurance de la simulation retenue (R-CRE-5) ;
  - **épargne restante** = soldes des comptes de l'épargne de précaution (R-CPT-13) − apport.
  Un taux d'endettement au-delà de **35 %**, ou une durée au-delà de **25 ans** (27 ans pour un achat dans le
  neuf), est **signalé** : ce sont les normes du HCSF, indicatives ; rien n'est bloqué. Sans revenus, le taux
  d'endettement n'est pas calculé.
- **R-CRE-16** — Le **co-emprunteur** est facultatif : un nom, des revenus et des charges mensuels saisis à la
  main. Il n'est pas un utilisateur de l'application (ADR-0006, Q19, Q21).

### Pièces à fournir

- **R-CRE-17** — La liste des **pièces à fournir** dépend de la demande :

  | Pièce | Nombre | Pour | Trouvée dans la bibliothèque |
  |---|---|---|---|
  | Pièce d'identité | 1 | chaque emprunteur | type pièce d'identité |
  | Justificatif de domicile de moins de 3 mois | 1 | l'emprunteur | type justificatif de domicile, date de référence de moins de 3 mois |
  | 3 dernières fiches de paie | 3 | chaque emprunteur | fiches de paie des emplois en cours, les plus récentes |
  | Contrat de travail | 1 | chaque emprunteur | contrat rattaché à un emploi en cours |
  | 2 derniers avis d'imposition | 2 | l'emprunteur | type document fiscal, les plus récents |
  | 3 derniers relevés de chaque compte courant | 3 par compte | l'emprunteur | relevés de chaque compte courant ouvert, les plus récents |
  | Relevé de l'épargne qui finance l'apport | 1 | si apport | relevé d'un compte d'épargne, le plus récent |
  | Offre ou tableau d'amortissement de chaque prêt en cours | 1 par prêt | si prêts en cours | offre de prêt ou tableau d'amortissement rattaché au prêt |
  | Compromis de vente | 1 | achat (ancien ou neuf) | compromis de vente rattaché au bien |
  | Devis des travaux | 1 | si travaux | devis rattaché au bien |

  Les pièces du co-emprunteur ne sont pas cherchées : on les **attribue** à la main.
- **R-CRE-18** — Pour chaque pièce, la demande montre les documents **attribués** à la main (n'importe quel
  document de la bibliothèque), sinon les documents **trouvés** selon la dernière colonne. Une pièce est
  **fournie** quand elle a son nombre de documents, **incomplète** en deçà, **manquante** sans aucun. Une
  pièce peut être marquée **non requise** (la banque ne la demande pas).
- **R-CRE-19** — **Export du dossier** : une archive ZIP qui range chaque document fourni dans un dossier par
  pièce (`01 Pièce d'identité/…`), sous son nom de fichier d'origine, avec la synthèse de la demande en HTML
  (`Synthèse.html`). La synthèse se consulte aussi dans l'application et s'imprime (PDF par l'impression du
  navigateur) : projet, plan de financement, revenus, charges, capacité, simulation retenue, liste des pièces
  et leur état. Les pièces manquantes sont signalées avant l'export ; elles ne l'empêchent pas.

### Statut et démarches

- **R-CRE-20** — Le **statut** d'une demande : `PREPARING` (en préparation), `SUBMITTED` (envoyée),
  `ACCEPTED` (une offre acceptée), `ABANDONED` (abandonnée). Il passe à `SUBMITTED` à la première démarche
  envoyée et à `ACCEPTED` quand une offre est acceptée ; sinon il se change à la main.
- **R-CRE-21** — Une **démarche** (`LenderRequest`) est l'envoi de la demande à un établissement ou un
  courtier : l'établissement (R-CPT-2) ou un nom libre, un **statut** (`SENT` envoyée, `OFFER` offre reçue,
  `REFUSED` refusée, `ACCEPTED` acceptée, `WITHDRAWN` retirée), la date d'envoi, la date de réponse, la date
  de **prochaine relance** (R-CRE-25) et une note. Une offre reçue s'enregistre comme une **simulation** de la
  demande, rattachée à la démarche (taux, durée, assurance, frais de l'offre).
- **R-CRE-22** — Les **offres reçues** se comparent comme des simulations (R-CRE-6). **Accepter** une offre
  passe sa démarche à `ACCEPTED`, les autres démarches en cours à `WITHDRAWN`, et la demande à `ACCEPTED`, puis
  ouvre l'assistant d'acceptation (R-CRE-27).
- **R-CRE-23** — Supprimer une demande supprime ses démarches, leurs journaux et ses attributions de pièces ; ses
  simulations restent, sans demande. Supprimer une simulation retire le lien des démarches qui la portaient.

### Suivi et relances

- **R-CRE-24** — Chaque démarche a un **journal** d'échanges (`LenderRequestEvent`) : une date, un type
  (`SENT` envoi, `FOLLOW_UP` relance, `CALL` appel, `MEETING` rendez-vous, `ANSWER` réponse, `NOTE` note) et un
  texte facultatif, du plus récent au plus ancien. Créer une démarche y inscrit son envoi ; enregistrer une
  offre, ou passer la démarche à refusée, une réponse. On ajoute, modifie et supprime les autres entrées à la
  main.
- **R-CRE-25** — La **prochaine relance** d'une démarche est une date facultative, proposée à la date d'envoi +
  14 jours. **Relancer** inscrit une relance au journal, datée du jour, et propose la prochaine relance 7 jours
  plus tard ; une réponse (offre reçue, refus) l'efface.
- **R-CRE-26** — Une démarche **à relancer** est une démarche envoyée ou avec une offre reçue dont la prochaine
  relance est passée ou du jour. Elle est signalée sur la fiche de la demande et dans la liste des demandes, et
  une **notification** l'annonce une fois par date de relance (centre de notifications, BUD-08), évaluée au
  démarrage du serveur, après chaque écriture et chaque jour.

### Acceptation

- **R-CRE-27** — L'**assistant d'acceptation**, ouvert après l'acceptation d'une offre (R-CRE-22), en trois
  étapes qu'on peut passer :
  1. **Bien** : si la demande n'a pas de bien, il en crée un (R-BIE-1) : nom de la demande, type d'après le
     projet (bien immobilier pour un achat ou des travaux, véhicule, autre), prix d'achat = prix du plan de
     financement, date d'achat à saisir ; la demande est rattachée au bien. Si elle en a un, il est repris ;
  2. **Prêt** : la simulation de l'offre est transformée en prêt (R-CRE-7), financé par le bien ;
  3. **Documents** : les documents du dossier sont proposés, cochés, pour être rattachés : offre de prêt,
     tableau d'amortissement et contrat d'assurance au prêt ; compromis de vente, acte de propriété, devis,
     diagnostics et garanties au bien. Sont proposés les documents de ces types attribués ou trouvés pour les
     pièces de la demande (R-CRE-18) et ceux de ces types qui ne sont rattachés à rien. On peut aussi y envoyer
     l'offre de prêt reçue, rattachée d'office au prêt.

## Capacité d'achat

- **R-CRE-30** — **Hypothèses** de la capacité d'achat, préremplies et modifiables, gardées d'une visite à
  l'autre : type de projet (R-CRE-10), taux nominal, durée, assurance, frais de dossier et de garantie, **taux
  d'endettement visé** (35 % par défaut), **réserve de sécurité** en mois de dépenses (6 par défaut), **comptes
  mobilisables** pour l'apport (par défaut ceux de l'épargne de précaution, R-CPT-13 ; tout compte ouvert peut
  être coché), autres revenus et charges mensuels, co-emprunteur (revenus et charges), prêts exclus des charges
  (R-CRE-14).
- **R-CRE-31** — **Aujourd'hui** :
  - revenus et charges mensuels comme une demande (R-CRE-13, R-CRE-14) ;
  - **mensualité maximale** = taux d'endettement visé × revenus − charges, jamais négative ;
  - **capital empruntable** avec cette mensualité, aux taux, durée et assurance des hypothèses (R-CRE-5) ;
  - **épargne mobilisable** = soldes des comptes cochés (valeur pour l'épargne variable, R-CPT-6) ;
  - **réserve** = nombre de mois × dépenses mensuelles moyennes (R-TDB-8) ;
  - **apport possible** = épargne mobilisable − réserve, jamais négatif ;
  - **budget** = capital empruntable + apport possible − frais de dossier − frais de garantie ;
  - **prix d'achat maximal** : le plus grand prix, à la centaine d'euros, dont le prix et les frais de notaire
    estimés (R-CRE-12) tiennent dans le budget.
- **R-CRE-32** — **Effet de l'achat**, au prix maximal : épargne restante (mobilisable − apport) et ce qu'elle
  représente en mois de dépenses ; reste à vivre (revenus − charges − échéance du crédit) ; **capacité
  d'épargne** mensuelle (R-PRJ-42, scénario par défaut, mois prochain) **avant** et **après** l'achat (après =
  avant − échéance du crédit + échéances des prêts exclus).
- **R-CRE-33** — **Dans les années à venir** : pour chacune des 5 prochaines années, à la même date, les mêmes
  calculs avec les données projetées du scénario par défaut (spec 07) :
  - revenus = moyenne des salaires projetés (R-PRJ-43, hausse du scénario) sur les 12 mois qui suivent, plus
    les autres revenus et ceux du co-emprunteur ;
  - charges = échéances des prêts en cours à cette date (prêts exclus retirés), plus les autres charges ;
  - épargne mobilisable = celle d'aujourd'hui + capacité d'épargne cumulée jusqu'à cette date ;
  - réserve = nombre de mois × dépenses projetées (R-PRJ-44) à cette date.
  Ce sont des hypothèses, pas des prévisions. Si le scénario par défaut n'a pas de capacité d'épargne
  (R-PRJ-47), seule l'année en cours est calculée.
- **R-CRE-34** — **Sensibilité** : le prix d'achat maximal d'aujourd'hui pour des durées de 15, 20 et 25 ans
  (27 dans le neuf) et des taux de 0,5 point en dessous, égal et 0,5 point au-dessus du taux des hypothèses.
- **R-CRE-35** — **Patrimoine projeté avec l'achat**, sur 10 ans, à côté de celui du scénario par défaut :
  le patrimoine projeté, diminué au mois de l'achat des frais (notaire, dossier, garantie) et, chaque mois, du
  cumul des intérêts et de l'assurance du crédit. Le bien garde son prix (R-BIE-5) : l'apport et le capital
  remboursé changent de forme sans appauvrir. Approximation affichée comme telle : l'apport ne rapporte plus
  d'intérêts, ce que la courbe n'en retire pas.
- **R-CRE-36** — **Préparer une demande** depuis une simulation enregistrée : une demande (R-CRE-10) du type
  de projet des hypothèses de la capacité, dont la simulation est retenue (et rattachée à la demande). Son plan
  de financement : le plus grand prix (R-CRE-31) qui tient dans le capital de la simulation + l'apport
  possible − les frais de la simulation, ses frais de notaire estimés, les frais de dossier et de garantie en
  autres frais, l'**apport nécessaire** (coût du projet − capital, au plus l'apport possible : le prix arrondi à
  la centaine laisse parfois un reliquat d'épargne), et les autres revenus et charges, le co-emprunteur et les
  prêts exclus des hypothèses. Tout se modifie ensuite. « Créer une demande » depuis la capacité d'achat
  enregistre d'abord une simulation au capital empruntable, aux conditions des hypothèses.
- **R-CRE-37** — Le **simulateur** de l'onglet Simuler part des hypothèses de la capacité (taux, durée,
  assurance, frais) et, en cherchant la mensualité, du capital empruntable ; on les change librement, sans
  modifier les hypothèses de la capacité.

## Features

| ID | Priorité | Feature |
|---|---|---|
| CRE-01 | V1 | Simuler un crédit : mensualité, coût total, TAEG, tableau ; enregistrer, comparer |
| CRE-02 | V1 | Calculs inverses : capital empruntable selon une mensualité, durée selon une mensualité |
| CRE-03 | V1 | Transformer une simulation en prêt |
| CRE-04 | V1 | Demande de crédit : plan de financement et capacité d'emprunt (co-emprunteur saisi) |
| CRE-05 | V1 | Pièces à fournir reliées à la bibliothèque ; export ZIP et synthèse imprimable |
| CRE-06 | V1 | Démarches auprès des banques et courtiers, offres reçues et comparées, acceptation |
| CRE-07 | V1 | Capacité d'achat : prix maximal aujourd'hui et dans les années à venir, effet sur l'épargne et le patrimoine |
| CRE-08 | V1 | Onglet Simuler : capacité et simulateur réunis ; préparer une demande depuis une simulation |
| CRE-09 | V1 | Journal des démarches, relances et rappels |
| CRE-10 | V1 | Acceptation d'une offre : bien, prêt et documents en une fois |

Exemple commun : aujourd'hui 2026-10-29. Emploi en cours à 2 700 € net, 13e mois récurrent de 2 600 € ; un
prêt auto avec tableau à 230 € d'échéance ; épargne de précaution 60 000 €.

### CRE-01 — Simulation

- 200 000 € à 3 % sur 240 mois, assurance 0,30 % du capital, 1 000 € de frais de dossier, 2 000 € de
  garantie : mensualité 1 109,20 €, assurance 50,00 €, intérêts 66 208,00 € (somme du tableau), coût total
  66 208,00 + 12 000,00 + 3 000,00 = 81 208,00 €. Le TAEG dépasse le taux nominal.
- Sans assurance ni frais, à 3 % nominal, le TAEG affiché est 3,04 %.

### CRE-02 — Calculs inverses

- Mensualité visée 1 200 € assurance comprise, 3 % sur 240 mois, assurance 0,30 % : le capital empruntable
  est le plus grand capital dont la mensualité et l'assurance ne dépassent pas 1 200 €.
- 200 000 € à 3 %, sans assurance, mensualité visée 1 400 € : 177 mois (1 399,71 € ; à 176 mois, la mensualité dépasse
  1 400 €).

### CRE-04 — Capacité

- Revenus : 2 700 + 2 600 / 12 = 2 916,67 € ; charges : 230 € ; avec une mensualité envisagée de 1 159,20 €,
  endettement (230 + 1 159,20) / 2 916,67 = 47,6 %, signalé ; avec un co-emprunteur à 2 500 € de revenus, il
  tombe à 25,6 %.
- Achat dans l'ancien à 280 000 € : frais de notaire estimés 21 000 €.

### CRE-05 — Pièces

- Avec trois fiches de paie archivées (août, septembre, octobre 2026), la pièce « 3 dernières fiches de
  paie » est fournie ; sans avis d'imposition, « 2 derniers avis d'imposition » est manquante.
- Un compromis de vente attribué à la main à la pièce la rend fournie. L'export contient
  `09 Compromis de vente/compromis.pdf` et `Synthèse.html`.

### CRE-06 — Démarches

- J'envoie la demande à LCL et au Crédit Agricole : la demande passe à « envoyée ». LCL répond par une offre
  à 3,35 % sur 25 ans ; je l'enregistre ; elle se compare aux simulations. Je l'accepte : la démarche Crédit
  Agricole passe à « retirée », la demande à « acceptée », et je transforme l'offre en prêt.

### CRE-07 — Capacité d'achat

- Revenus 2 916,67 €, prêt auto 230 €, taux visé 35 % : mensualité maximale 1 020,83 − 230 = 790,83 €. À 3 %
  sur 20 ans sans assurance ni frais, le capital empruntable est de 142 596 €.
- Épargne de précaution 60 000 €, dépenses moyennes 2 000 € : réserve de 6 mois 12 000 €, apport possible
  48 000 €. Budget 142 596 + 48 000 = 190 596 € ; dans l'ancien, prix maximal 177 200 € (177 200 + 13 290 de
  notaire ≤ 190 596 ; à 177 300 €, 177 300 + 13 298 le dépasse).
- Avec une hausse des salaires de 2 % par an et une capacité d'épargne de 600 € par mois, le prix maximal de
  l'an prochain est plus élevé : revenus et épargne mobilisable ont augmenté.
- L'épargne restante est la réserve, 12 000 € soit 6 mois de dépenses ; la capacité d'épargne passe de 600 € à
  600 − 790,83 = −190,83 € par mois, signalée.

### CRE-09 — Relances

- J'envoie la demande à LCL le 2026-10-20 : le journal note l'envoi, la prochaine relance est proposée au
  2026-11-03. Le 2026-11-03, la démarche est « à relancer » et une notification l'annonce, une seule fois.
- Je relance le 2026-11-04 : le journal note la relance ; la prochaine relance passe au 2026-11-11.
- LCL envoie une offre : le journal note la réponse, la relance est effacée.

### CRE-10 — Acceptation

- J'accepte l'offre LCL de la demande « Maison Annecy » (sans bien) : l'assistant crée le bien « Maison
  Annecy », bien immobilier à 420 000 €, acheté le 2027-01-15, puis le prêt de l'offre financé par ce bien ; le
  compromis attribué à la pièce « Compromis de vente » est rattaché au bien, et l'offre de prêt envoyée dans
  l'assistant au prêt.
