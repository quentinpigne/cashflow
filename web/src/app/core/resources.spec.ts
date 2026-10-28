import { Resource, signal } from '@angular/core';

import { keptValue } from './resources';

/** Ressource réduite à ce que lit `keptValue`. */
function fakeResource<T>(initial: T | undefined) {
  const value = signal(initial);
  const resource = {
    value,
    hasValue: () => value() !== undefined,
  } as unknown as Resource<T | undefined>;
  return { resource, value };
}

describe('keptValue', () => {
  it('garde la valeur pendant un rechargement de la même clé', () => {
    const { resource, value } = fakeResource<string>('v1');
    const key = signal('a');
    const kept = keptValue(resource, key);
    expect(kept()).toBe('v1');
    value.set(undefined);
    expect(kept()).toBe('v1');
    value.set('v2');
    expect(kept()).toBe('v2');
  });

  it('oublie la valeur quand la clé change', () => {
    const { resource, value } = fakeResource<string>('v1');
    const key = signal('a');
    const kept = keptValue(resource, key);
    expect(kept()).toBe('v1');
    key.set('b');
    value.set(undefined);
    expect(kept()).toBeUndefined();
  });
});
