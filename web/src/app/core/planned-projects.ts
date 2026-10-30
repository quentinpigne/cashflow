import { Injectable, signal } from '@angular/core';

const STORAGE_KEY = 'cashflow.planned-projects';

function stored(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) !== 'false';
  } catch {
    return true;
  }
}

/**
 * Interrupteur « Projets en projet » du budget du mois et de l'évolution (R-BUD-72) : activé par défaut,
 * mémorisé sur l'appareil.
 */
@Injectable({ providedIn: 'root' })
export class PlannedProjects {
  readonly value = signal(stored());

  set(value: boolean): void {
    this.value.set(value);
    try {
      localStorage.setItem(STORAGE_KEY, String(value));
    } catch {
      // Stockage indisponible : le choix vaut pour la session.
    }
  }
}
