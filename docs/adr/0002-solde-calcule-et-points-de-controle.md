# ADR-0002 — Solde calculé et points de contrôle

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : specs 01, 04 ; modèle de données

## Contexte

Une approche courante consiste à stocker un solde courant et à l'ajuster à chaque création,
modification, suppression ou import de transaction. Chaque chemin de code doit alors penser à
l'ajuster ; un oubli (suppression de compte, import interrompu) fait dériver le solde sans aucun moyen
de le détecter.
Le besoin de **vérifier** les transactions contre les relevés impose par ailleurs de connaître le
solde à n'importe quelle date passée.

## Options envisagées

1. **Solde stocké et mis à jour incrémentalement** — ✅ lecture immédiate. ❌ Dérive silencieuse, pas
   de solde historique, logique dupliquée dans tous les cas d'usage.
2. **Solde calculé** (`opening_balance + Σ transactions ≤ D`) — ✅ toujours juste, solde à toute date.
   ❌ Coût d'agrégation, négligeable à l'échelle d'un particulier (quelques dizaines de milliers de lignes).
3. **Solde calculé, avec cache dérivé** — comme 2, en ajoutant un cache reconstructible si les
   performances l'exigent un jour.

## Décision

Le solde est **calculé**. Aucun champ « solde courant » n'est une source de vérité. Un cache
(soldes mensuels matérialisés, par exemple) est permis s'il est mesuré comme nécessaire, s'il est
reconstructible à tout moment, et s'il est vérifié par un test de cohérence.

La vérification contre la banque repose sur des **points de contrôle** (`BalanceCheckpoint`) :
le solde réel à une date, comparé au solde calculé. Un point de contrôle sans écart peut être
**rapproché**, ce qui verrouille les transactions qu'il couvre (spec 04).

## Conséquences

- Supprimer ou modifier une transaction n'a pas d'effet de bord sur le compte.
- Le solde historique, la courbe de solde et le patrimoine à date sont gratuits.
- Le verrouillage par rapprochement devient indispensable : sans lui, une modification ancienne
  ferait bouger silencieusement un solde que la banque a confirmé.
