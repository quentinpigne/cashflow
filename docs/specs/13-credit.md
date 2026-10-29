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

Rien de tout cela n'entre dans le patrimoine, les budgets ou les projections : seul le prêt créé à partir
d'une simulation compte, comme tout prêt (spec 09).

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
  `REFUSED` refusée, `ACCEPTED` acceptée, `WITHDRAWN` retirée), la date d'envoi, la date de réponse et une
  note. Une offre reçue s'enregistre comme une **simulation** de la demande, rattachée à la démarche (taux,
  durée, assurance, frais de l'offre).
- **R-CRE-22** — Les **offres reçues** se comparent comme des simulations (R-CRE-6). **Accepter** une offre
  passe sa démarche à `ACCEPTED`, les autres démarches en cours à `WITHDRAWN`, et la demande à `ACCEPTED` ;
  on propose alors de **transformer** sa simulation en prêt (R-CRE-7), rattaché au bien de la demande.
- **R-CRE-23** — Supprimer une demande supprime ses démarches et ses attributions de pièces ; ses simulations
  restent, sans demande. Supprimer une simulation retire le lien des démarches qui la portaient.

## Features

| ID | Priorité | Feature |
|---|---|---|
| CRE-01 | V1 | Simuler un crédit : mensualité, coût total, TAEG, tableau ; enregistrer, comparer |
| CRE-02 | V1 | Calculs inverses : capital empruntable selon une mensualité, durée selon une mensualité |
| CRE-03 | V1 | Transformer une simulation en prêt |
| CRE-04 | V1 | Demande de crédit : plan de financement et capacité d'emprunt (co-emprunteur saisi) |
| CRE-05 | V1 | Pièces à fournir reliées à la bibliothèque ; export ZIP et synthèse imprimable |
| CRE-06 | V1 | Démarches auprès des banques et courtiers, offres reçues et comparées, acceptation |

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
- 200 000 € à 3 %, sans assurance, mensualité visée 1 400 € : 182 mois (la mensualité à 181 mois dépasse
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
