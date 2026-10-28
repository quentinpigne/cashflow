import { Resource, Signal, linkedSignal } from '@angular/core';

/**
 * Valeur d'une ressource qui reste affichée pendant son rechargement, tant que `key` ne change pas.
 *
 * Une ressource dont les paramètres changent (par exemple à chaque `DataVersion.bump()`) repasse à
 * `undefined` le temps de la requête : un écran qui l'affiche sous un `@if` se vide, se reconstruit
 * et revient en haut. Avec une autre clé (un autre compte, un autre import), la valeur est oubliée.
 */
export function keptValue<T, K>(
  resource: Resource<T | undefined>,
  key: () => K,
): Signal<T | undefined> {
  return linkedSignal<{ key: K; value: T | undefined }, T | undefined>({
    source: () => ({ key: key(), value: resource.hasValue() ? resource.value() : undefined }),
    computation: (source, previous) =>
      source.value ?? (previous?.source.key === source.key ? previous.value : undefined),
  });
}
