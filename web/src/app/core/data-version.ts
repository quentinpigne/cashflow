import { Injectable, signal } from '@angular/core';

/**
 * Version des données : incrémentée après chaque modification, elle relance les ressources qui en
 * dépendent (comptes, transactions, accueil). Une seule source, pour que tout l'écran reste cohérent.
 */
@Injectable({ providedIn: 'root' })
export class DataVersion {
  private readonly counter = signal(0);
  readonly value = this.counter.asReadonly();

  bump(): void {
    this.counter.update((v) => v + 1);
  }
}
