# ADR-0030 — Recherche d'adresse à l'étranger par Photon

- **Statut** : Accepté
- **Date** : 2026-11-10
- **Concerne** : spec 11, écrans (E7, E14, E31, E32, E9), architecture ; complète l'ADR-0025

## Contexte

La recherche d'adresse passe par la Base Adresse Nationale (ADR-0025), qui ne couvre que la France. Une
dépense à l'étranger (vacances, déplacement) se saisit donc à la main, sans coordonnées : elle figure dans
l'analyse par pays et par ville, mais ne peut pas être localisée. On veut pouvoir rechercher une adresse
dans un autre pays, avec la même saisie : on tape, on choisit une proposition. Les contraintes de
l'ADR-0025 restent : appel par le serveur seulement, désactivé par défaut (ADR-0007), aucun secret si
possible, aucun flottant (ADR-0001). La BAN reste la meilleure source pour la France (numéros de rue,
codes postaux officiels).

## Options envisagées

1. **Photon** (komoot, `photon.komoot.io`), pour les pays autres que la France — moteur de recherche
   d'adresses sur les données OpenStreetMap. ✅ mondial, gratuit, sans clé, conçu pour la recherche au fil
   de la frappe, réponse GeoJSON proche de celle de la BAN, filtre par pays (`countrycode`), noms en
   français (`lang=fr`) / ❌ instance publique sans garantie de disponibilité, à utiliser « dans une limite
   raisonnable » (largement le cas d'un usage personnel) ; un second service tiers.
2. **Photon pour tous les pays**, BAN abandonnée. ✅ un seul service / ❌ moins précis que la BAN pour
   les adresses françaises, qui sont l'essentiel des dépenses.
3. **Nominatim** (OpenStreetMap), pour l'étranger. ✅ service officiel d'OpenStreetMap / ❌ sa charte
   interdit la recherche au fil de la frappe : il faudrait un bouton « Rechercher », une requête par seconde
   au plus.
4. **Service commercial** (Google, Mapbox, OpenCage…). ✅ qualité / ❌ clé d'API à gérer (un secret),
   quotas, compte à créer.

## Décision

- La recherche d'adresse **suit le pays choisi** : France → **BAN** (inchangé) ; tout autre pays →
  **Photon**, restreint à ce pays (`countrycode`), noms en français (`lang=fr`). Le pays se choisit donc
  **avant** la recherche : il passe en tête de la section Adresse.
- Une proposition de Photon remplit les mêmes champs qu'une proposition de la BAN : ligne d'adresse (voie
  et numéro, dans l'ordre d'usage du pays ; à défaut, le nom du lieu), code postal, ville, pays et
  coordonnées (lues sous leur forme écrite, sans flottant). Une proposition sans ville (une région, un pays)
  est écartée ; un champ plus long que ce que l'adresse admet est laissé vide.
- Un **seul réglage** (`geocoding.enabled`) active la recherche d'adresse, pour les deux services ; son
  explication les nomme tous les deux, et le champ de recherche indique le service appelé pour le pays
  choisi (ADR-0007), avec la mention des données OpenStreetMap pour Photon.
- L'adresse du service est configurable (`CASHFLOW_GEOCODING_WORLD_URL`, `https://photon.komoot.io` par
  défaut), par exemple pour une instance de Photon hébergée chez soi. Aucun secret.
- La **carte** ne change pas : elle reste celle de la France métropolitaine (ADR-0025). Une adresse à
  l'étranger localisée n'y figure pas, et compte parmi les lieux absents de la carte (R-TIE-29).

## Conséquences

- Une adresse à l'étranger se trouve et se localise comme une adresse française ; elle garde ses
  coordonnées, prêtes pour une future carte du monde.
- Un second service tiers, sous le même interrupteur : l'activer envoie le texte saisi à la BAN pour la
  France, à Photon pour l'étranger, jamais aux deux.
- Instance publique sans garantie : injoignable, la recherche échoue proprement et la saisie à la main
  reste possible, comme pour la BAN.
- Changer de pays efface les coordonnées, comme toute correction à la main (R-TIE-25).
