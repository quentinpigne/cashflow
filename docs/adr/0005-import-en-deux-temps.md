# ADR-0005 — Import en deux temps et dédoublonnage par empreinte

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : spec 03, spec 04

## Contexte

Écrire directement en base les transactions lues dans un fichier ne permet ni aperçu, ni
dédoublonnage, ni annulation. Or l'import doit aussi servir à **vérifier** des transactions saisies à
la main, ce qui exige une décision humaine sur les correspondances.

## Options envisagées

1. **Import direct** — ❌ Ce sont les problèmes ci-dessus.
2. **Import en deux temps : zone de revue (`ImportBatch`/`ImportLine`), puis validation atomique** —
   ✅ aperçu, correction, décisions par ligne, annulation. ❌ Plus d'entités et d'écrans.

Pour le dédoublonnage :
- a. **Comparaison floue** (date ± n, montant, libellé) — ❌ faux positifs sur les achats identiques
  répétés.
- b. **Empreinte déterministe par ligne** avec un rang d'occurrence dans le fichier — ✅ idempotent pour
  un même fichier, distingue deux achats identiques le même jour. ❌ Sensible aux banques qui changent
  leurs libellés entre deux exports.
- c. **Identifiant fourni par la banque** (`FITID` de l'OFX) — ✅ le plus fiable, mais rarement disponible.

## Décision

- Import en **deux temps** (option 2) : rien n'est modifié avant la validation, qui est atomique ;
  un lot validé est annulable tant qu'il n'est pas rapproché.
- Dédoublonnage par **identifiant bancaire** quand il existe (c), sinon par **empreinte déterministe**
  (b). La comparaison floue (a) ne sert qu'à **proposer** des correspondances avec des transactions
  existantes non importées, jamais à décider seule.

## Conséquences

- Réimporter un fichier est sans danger : c'est une propriété testée.
- Les libellés instables (R-IMP-8) produisent des candidats à valider à la main plutôt que des doublons
  silencieux.
- Les lignes d'import sont conservées : la traçabilité « transaction ← ligne n du fichier X » est complète.
