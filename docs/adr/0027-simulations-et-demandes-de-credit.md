# ADR-0027 — Simulations et demandes de crédit

- **Statut** : Accepté
- **Date** : 2026-10-29
- **Concerne** : spec 13, spec 09, spec 12, modèle de données, ADR-0001, ADR-0008, ADR-0020, ADR-0024

## Contexte

La rubrique Prêts suit les prêts en cours (spec 09). On veut aussi y préparer un nouveau crédit : simuler
plusieurs hypothèses, monter le dossier demandé par les banques (plan de financement, taux d'endettement,
pièces justificatives) et suivre les réponses des banques et courtiers jusqu'à l'offre acceptée.

Les banques demandent toujours les mêmes informations, que l'application connaît en grande partie : salaires
(spec 10), prêts en cours (spec 09), épargne (spec 01), et désormais les documents (spec 12). Le dossier peut
réunir deux emprunteurs alors que l'application n'en suit qu'un (ADR-0006).

## Options envisagées

**Simulations**

1. **Calcul à la volée, rien d'enregistré** — ✅ simple / ❌ une offre reçue ou une hypothèse comparée se perd
   en quittant l'écran ; pas de lien avec la demande ni avec le prêt créé.
2. **Simulations enregistrées** — ✅ comparables, rattachées à une demande, transformables en prêt ; une offre
   reçue est une simulation comme une autre / ❌ une table de plus.

**Pièces à fournir**

3. **Liste saisie par l'utilisateur** — ✅ libre / ❌ tout à refaire à chaque demande ; rien n'est trouvé seul.
4. **Catalogue fixe dans le domaine**, documents trouvés dans la bibliothèque par des règles, attribution à la
   main en complément — ✅ la liste usuelle des banques, préremplie / ❌ le catalogue évolue par le code.

**Export**

5. **PDF généré par le serveur** — ✅ un seul fichier / ❌ une bibliothèque de mise en page à embarquer.
6. **Archive ZIP** des pièces rangées par dossier, avec une synthèse HTML, imprimable en PDF par le navigateur
   — ✅ les pièces restent les fichiers d'origine ; aucune mise en page côté serveur / ❌ deux gestes pour
   obtenir un PDF de la synthèse.

## Décision

Options **2**, **4** et **6** (décision du propriétaire, 2026-10-29).

- **Simulations** : entité `LoanSimulation` (type, capital, taux, durée, assurance, frais de dossier et de
  garantie), rattachable à une demande, à un établissement, et au prêt qui en est issu. Le calcul réutilise le
  tableau d'amortissement de l'ADR-0020 ; la première échéance est fixée un mois après la date du jour.
- **TAEG** : taux actuariel calculé par **dichotomie en décimal** (`rust_decimal`), sans flottant (ADR-0001),
  au millionième près ; frais et assurance compris. La dichotomie est bornée (0 à 100 % par mois) et converge
  en moins de 100 itérations.
- **Calculs inverses** : capital empruntable par dichotomie sur le capital (à l'euro), durée par parcours des
  durées de 1 à 600 mois ; le domaine les expose comme fonctions pures.
- **Demande de crédit** : entité `LoanApplication` (projet, plan de financement, autres revenus et charges,
  co-emprunteur saisi, prêts exclus des charges, simulation retenue, statut). Revenus, charges, taux
  d'endettement, reste à vivre et épargne restante sont **calculés** à chaque lecture, jamais stockés
  (ADR-0002). Les seuils du HCSF (35 % d'endettement, 25 ans, 27 dans le neuf) sont des constantes du domaine,
  affichés comme indicatifs : rien n'est bloqué.
- **Co-emprunteur** : nom, revenus et charges saisis dans la demande ; ce n'est pas un membre du foyer et ses
  pièces s'attribuent à la main. Le multi-membres (Q19, Q21) le remplacera peut-être.
- **Pièces** : catalogue fixe (R-CRE-17) ; une table `loan_application_piece` garde les attributions à la
  main et les pièces marquées non requises. Les documents trouvés ne sont pas stockés : ils se recalculent.
- **Démarches** : entité `LenderRequest` par établissement ou courtier, avec son statut ; une offre reçue est
  la simulation rattachée à la démarche.
- **Export** : `GET /loan-applications/{id}/export` produit l'archive ZIP (crate `zip`, pur Rust) ; la
  synthèse HTML est rendue par le serveur à partir d'un gabarit simple, sans moteur de gabarits.
- **Types de documents** (précise la spec 12) : acte de propriété, compromis de vente, certificat
  d'immatriculation, diagnostic, devis, garantie, offre de prêt, tableau d'amortissement, contrat
  d'assurance, pièce d'identité, justificatif de domicile. La migration recrée la table `document`
  (procédure de l'ADR-0017).

## Conséquences

- Les simulations, demandes, démarches et attributions entrent dans l'export JSON (ADR-0008).
- Le prêt créé depuis une simulation est un prêt ordinaire : rien du crédit simulé n'entre ailleurs dans le
  patrimoine ni dans les projections.
- Ajouter une pièce au catalogue ou un type de projet passe par le code (et, pour un type, une migration).
- L'export ZIP est le premier usage d'une archive dans l'application : l'option « inclure les fichiers » de
  l'export complet (ADR-0008) pourra réutiliser la même dépendance.
