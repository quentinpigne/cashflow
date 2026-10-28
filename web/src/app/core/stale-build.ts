import { NavigationError } from '@angular/router';

const RELOADED_KEY = 'cashflow.reloadedFor';

/**
 * Un onglet ouvert avant un déploiement référence des modules qui n'existent plus : la navigation vers un
 * écran chargé à la demande échoue. On recharge alors la page sur l'écran demandé, une seule fois par URL
 * pour ne pas boucler si le serveur est réellement en panne.
 */
export function reloadOnStaleBuild(event: NavigationError): void {
  if (!isModuleLoadError(event.error)) {
    return;
  }
  try {
    if (sessionStorage.getItem(RELOADED_KEY) === event.url) {
      return;
    }
    sessionStorage.setItem(RELOADED_KEY, event.url);
  } catch {
    return;
  }
  window.location.assign(event.url);
}

/** Messages de Chrome, Firefox et Safari pour un `import()` dynamique échoué. */
function isModuleLoadError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return /dynamically imported module|Importing a module script failed|module script/i.test(
    message,
  );
}
