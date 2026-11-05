import { Location } from '@angular/common';
import { Directive, Injectable, computed, inject, input } from '@angular/core';
import { Router, UrlTree } from '@angular/router';

/**
 * Retour arrière dans l'application : revient à la page précédente de l'historique, avec son état
 * (filtres, page), et ne se replie sur `fallback` que si la page a été ouverte directement (lien
 * collé, nouvel onglet) : l'historique ne contient alors rien de l'application.
 */
@Injectable({ providedIn: 'root' })
export class BackNavigation {
  private readonly router = inject(Router);
  private readonly location = inject(Location);

  /**
   * Le routeur numérote les pages de l'onglet dans l'état de l'historique
   * (`canceledNavigationResolution: 'computed'`, cf. app.config.ts) : la première vaut 0.
   */
  canGoBack(): boolean {
    const state = this.location.getState() as { ɵrouterPageId?: number } | null;
    return (state?.ɵrouterPageId ?? 0) > 0;
  }

  back(fallback: string | readonly unknown[] | UrlTree): void {
    if (this.canGoBack()) {
      this.location.back();
    } else if (fallback instanceof UrlTree || typeof fallback === 'string') {
      void this.router.navigateByUrl(fallback, { replaceUrl: true });
    } else {
      void this.router.navigate(fallback, { replaceUrl: true });
    }
  }
}

/** `<a mat-icon-button appBack="/reglages">` : flèche de retour (cf. `BackNavigation`). */
@Directive({
  selector: 'a[appBack]',
  host: { '[attr.href]': 'href()', '(click)': 'onClick($event)' },
})
export class BackLink {
  private readonly router = inject(Router);
  private readonly navigation = inject(BackNavigation);
  /** Page parente, si l'historique ne contient pas de page précédente. */
  readonly appBack = input.required<string | readonly unknown[]>();

  private readonly tree = computed(() => {
    const target = this.appBack();
    return typeof target === 'string'
      ? this.router.parseUrl(target)
      : this.router.createUrlTree(target as unknown[]);
  });
  protected readonly href = computed(() => this.router.serializeUrl(this.tree()));

  protected onClick(event: MouseEvent): void {
    // Un clic du milieu, ou avec une touche, ouvre la page parente dans un nouvel onglet.
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) {
      return;
    }
    event.preventDefault();
    this.navigation.back(this.tree());
  }
}
