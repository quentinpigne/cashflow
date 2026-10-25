# ADR-0013 — Cœur métier pur dans une architecture en couches

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : architecture, ADR-0009, ADR-0017

## Contexte

La valeur de l'application est dans des calculs exigeants : soldes, ventilations, intérêts par
quinzaine, TRI, dédoublonnage, correspondances, projections. Mêlés aux requêtes en base et au framework
web, ils deviennent difficiles à tester et à faire évoluer.

En face, le projet est petit : un seul utilisateur, une seule base (SQLite, sans changement prévu), une
seule interface (l'API REST). Le code est écrit par une IA et relu par le propriétaire du projet.

## Options envisagées

1. **Architecture hexagonale complète** (ports et adaptateurs) : cœur pur, cas d'usage génériques sur
   des traits (`AccountRepository`, `UnitOfWork`, `Clock`…), adaptateurs dans des crates séparées.
   ✅ Isolation maximale. ❌ Des ports qui n'auront jamais d'autre implémentation ; en Rust, traits
   asynchrones et génériques qui se propagent, beaucoup de code d'assemblage à écrire et à relire.
2. **N-tiers classique** (API → services → accès aux données). ✅ Simple. ❌ Rien n'empêche les calculs
   de se disperser dans les services ou dans le SQL : les specs ne se testent plus sans base.
3. **Couches avec un cœur de calcul pur** (*functional core, imperative shell*). ✅ La simplicité du
   n-tiers, et les calculs restent purs, testables règle par règle. ❌ Une discipline à tenir : les calculs
   vont dans le domaine.

## Décision

Option 3.

```
routes (axum : assemblage, middlewares)
  └─► handlers (extraction, DTO, OpenAPI)
        └─► services (un par cas d'usage)
              ├─► domain (pur : types, invariants, moteurs de calcul)
              └─► db (requêtes sqlx)
```

- **`domain`** : types (`Money`, `Rate`, identifiants), entités, invariants et **moteurs** sous forme de
  fonctions pures. Aucune dépendance vers la base, le système de fichiers, le réseau ou l'horloge : la
  date du jour est un paramètre.
- **`services`** : un service par cas d'usage. Il ouvre la transaction SQLite, charge les données, appelle
  le domaine, écrit le résultat. Pas de logique de calcul.
- **`db`** : les requêtes sqlx, appelées directement par les services, sans trait intermédiaire.
- **`handlers`** : handlers axum (extraction de la requête, appel d'un service, mise en forme de la
  réponse), DTO et description OpenAPI (ADR-0016). Ni calcul ni requête.
- **`routes`** : assemblage des handlers en routeurs par domaine, middlewares (session), contrat OpenAPI.
  Ce découpage suit celui d'axum et celui de Foodflow.
- **`cli`** : l'autre point d'entrée, au même niveau que `routes`. Il porte les sous-commandes du binaire
  (`serve`, sauvegarde, export, mot de passe, jetons…), un fichier par sous-commande, et appelle les
  services. `main.rs` ne fait qu'initialiser les journaux et lancer la ligne de commande.
- **Traits seulement là où plusieurs implémentations existent vraiment** : les lecteurs de relevés
  (CSV, XLSX, OFX, PDF) et l'OCR optionnel (ADR-0007).
- **Deux crates** : `domain` (pur) et `server` (routes, handlers, services, db, lecteurs de fichiers, binaire). La
  frontière de crate fait respecter la pureté par le compilateur : `domain` n'a ni tokio, ni sqlx, ni
  axum dans ses dépendances, et les flottants y sont interdits par clippy.
- Un agrégat SQL (par exemple Σ par compte) n'est qu'une **optimisation** d'un calcul du domaine ; un test
  de cohérence vérifie qu'il donne le même résultat que le moteur.

## Conséquences

- La majorité des tests porte sur le domaine, sans base ni mocks : rapides et déterministes. Les specs
  se traduisent presque littéralement en tests (une règle `R-*` → un ou plusieurs tests).
- Les services se testent sur une vraie base SQLite en mémoire, sans mocks.
- Peu de code d'assemblage : pas de ports pour la persistance ni l'horloge.
- Changer de base demanderait de réécrire `db` et une partie des services ; c'est un coût accepté, ce
  changement n'étant pas envisagé (ADR-0017).
