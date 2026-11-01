/** Tailles de page proposées sous les listes de transactions (E3, E14). */
export const PAGE_SIZES = [50, 100, 200];

const storageKey = (list: string) => `cashflow.page-size.${list}`;

/**
 * Taille de page retenue dans ce navigateur pour une liste ; `fallback` si rien n'est retenu, si la valeur
 * n'est pas proposée ou si le stockage est indisponible.
 */
export function loadPageSize(list: string, fallback: number): number {
  try {
    const stored = Number(localStorage.getItem(storageKey(list)));
    return PAGE_SIZES.includes(stored) ? stored : fallback;
  } catch {
    return fallback;
  }
}

/** Retient la taille de page choisie ; sans stockage, le choix vaut jusqu'au rechargement. */
export function savePageSize(list: string, size: number): void {
  try {
    localStorage.setItem(storageKey(list), String(size));
  } catch {
    // Stockage indisponible.
  }
}
