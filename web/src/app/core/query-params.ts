import { effect, inject } from '@angular/core';
import { ActivatedRoute, Params, Router } from '@angular/router';

/**
 * Recopie l'état d'une liste (filtres, tri, page) dans les paramètres de l'adresse, sans créer d'entrée
 * d'historique : le retour arrière retrouve la liste telle qu'on l'a laissée (ecrans.md, E3 et E14). Une valeur
 * `null` retire le paramètre. L'écriture est différée pour ne pas réécrire l'adresse à chaque frappe.
 *
 * À appeler dans un contexte d'injection (constructeur ou initialisation d'un champ).
 */
export function syncQueryParams(params: () => Params): void {
  const router = inject(Router);
  const route = inject(ActivatedRoute);
  effect((onCleanup) => {
    const queryParams = params();
    const timer = setTimeout(() => {
      // Une navigation en cours (ouverture d'une transaction) ne doit pas être remplacée.
      if (router.currentNavigation()) return;
      const target = router.createUrlTree([], { relativeTo: route, queryParams });
      if (router.serializeUrl(target) === router.url) return;
      void router.navigateByUrl(target, { replaceUrl: true });
    }, 300);
    onCleanup(() => clearTimeout(timer));
  });
}
