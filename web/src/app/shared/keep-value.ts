import { ResourceRef, Signal, computed } from '@angular/core';

/**
 * Dernière valeur chargée d'une ressource, gardée pendant qu'elle se recharge : un encadré de l'accueil ne
 * disparaît pas (et son menu de réglages ne se ferme pas) quand on change ses membres (R-MBR-8).
 */
export function keepValue<T>(resource: ResourceRef<T | undefined>): Signal<T | undefined> {
  let last: T | undefined;
  return computed(() => {
    const value = resource.hasValue() ? resource.value() : undefined;
    if (value !== undefined) last = value;
    return last;
  });
}
