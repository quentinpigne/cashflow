# Vision produit

Statut : **Validée** (2026-10-20)

## Le problème

Mes finances sont réparties entre plusieurs banques et plusieurs types de produits :
comptes courants, livrets, assurance vie, PEA, épargne retraite… Aucun outil ne me donne
à la fois :

- une vue **fiable** de chaque compte, vérifiée contre les relevés de la banque ;
- un suivi budgétaire où une dépense peut être **répartie** sur plusieurs postes ;
- une **projection** crédible de mon épargne, qu'elle soit à taux fixe ou à rendement variable.

## Pour qui

Un seul utilisateur : moi, depuis mon ordinateur et mon téléphone. Application personnelle,
mono-utilisateur (ADR-0006), hébergée sur un petit serveur personnel (ADR-0015).
Les données sont sensibles et ne doivent pas quitter mon contrôle sans action volontaire.

## Objectifs

1. **Tenir un registre exact** de tous mes comptes, à l'euro près, vérifiable à tout moment
   contre un relevé bancaire.
2. **Saisir peu, importer beaucoup** : importer des relevés (CSV à colonnes personnalisées,
   Excel, OFX, PDF) et vérifier des transactions saisies à la main avant que la banque ne les
   passe.
3. **Comprendre où va l'argent** : catégoriser, ventiler, comparer au budget.
4. **Piloter l'épargne** : savoir ce que rapportent mes placements et ce qu'ils vaudront dans
   1, 5, 20 ans selon plusieurs scénarios.

## Indicateurs de succès

- Pour chaque compte courant, l'écart entre le solde calculé et le relevé est de **0,00 €**
  à chaque date de relevé importé.
- Importer un relevé mensuel et le catégoriser prend **moins de 5 minutes**.
- Réimporter un fichier déjà importé ne crée **aucun doublon**.
- La projection d'un livret réglementé correspond **au centime près** aux intérêts versés
  par la banque, une fois les mêmes taux saisis.

## Principes

- **Exactitude avant fonctionnalités.** Un chiffre faux est pire qu'un chiffre absent.
- **Le relevé bancaire fait foi.** L'application calcule ; les relevés vérifient.
- **Rien ne se perd.** Tout import est traçable jusqu'à sa ligne d'origine ; on peut annuler un import.
- **Auto-hébergé.** Les données restent sur mon serveur, accessible uniquement depuis mon réseau
  local (ADR-0015). Tout envoi à un service tiers (OCR par exemple) est explicite et désactivable (ADR-0007).
- **Mes données sont exportables** à tout moment, dans un format ouvert (ADR-0008).

## Périmètre

| Domaine | Inclus | Spec |
|---|---|---|
| Comptes | Courants, espèces, livrets réglementés, épargne à taux fixe, assurance vie, PEA, PER, compte-titres | [01](specs/01-comptes.md) |
| Transactions | Saisie, catégories, ventilation, virements internes, récurrences | [02](specs/02-transactions.md) |
| Import | CSV/XLSX à mapping personnalisé, OFX, PDF (extraction locale, OCR en repli), règles de catégorisation | [03](specs/03-import.md) |
| Rapprochement | Points de contrôle de solde, statuts de pointage, verrouillage | [04](specs/04-rapprochement.md) |
| Budgets | Lignes mensuelles et annuelles par catégorie, report, projets multi-catégories, objectifs d'épargne, alertes | [05](specs/05-budgets.md) |
| Épargne | Taux fixe (intérêts calculés), taux variable (valorisations, performance) | [06](specs/06-epargne.md) |
| Projections | Trésorerie, épargne fixe, épargne variable par scénarios | [07](specs/07-projections.md) |
| Tableau de bord | Patrimoine, taux d'épargne, épargne de précaution, budgets | [08](specs/08-tableau-de-bord.md) |
| Biens et prêts | Biens, prêts, tableau d'amortissement | [09](specs/09-biens-et-prets.md) |
| Salaires | Emplois, rémunérations et primes déclarées, comparaison au reçu, projection, capacité d'épargne | [10](specs/10-salaires.md) |
| Tiers | Tiers des transactions, groupes de tiers (enseignes), dépenses par tiers et par groupe | [11](specs/11-tiers.md) |

## Non-objectifs (au moins pour la V1)

- **Multi-utilisateur.** La V1 sert une seule personne. À terme, chaque membre du foyer gérera ses
  comptes et son patrimoine, avec des comptes partagés (ADR-0006) : la V1 le prépare sans l'implémenter.
- **Synchronisation bancaire automatique** (DSP2 / agrégateurs). Coûteuse, fragile, et elle
  expose les identifiants. L'import de fichiers suffit.
- **Multi-devises.** Chaque compte a une devise (EUR par défaut), mais aucune conversion
  n'est faite. Un compte en devise étrangère est exclu des agrégats.
- **Suivi ligne à ligne des titres** (cours en temps réel, nombre de parts). L'épargne variable
  est suivie par valorisations saisies ou importées (ADR-0010). Le suivi des lignes pourra venir en V2.
- **Déclaration fiscale.** Les projections peuvent estimer un net de fiscalité, mais l'application
  ne remplace pas un conseil fiscal.
- **Prêts complexes.** La V1 calcule le tableau d'amortissement d'un prêt amortissable à taux fixe
  (ADR-0020) ; le différé, le remboursement anticipé, le taux variable et le prêt in fine viendront
  après la V1.
- **Score financier synthétique** : trop arbitraire.
