import { Injectable, inject, signal } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { AuthService } from '../api/services/auth.service';

/** Session de l'utilisateur unique (ADR-0015). */
@Injectable({ providedIn: 'root' })
export class Session {
  private readonly api = inject(AuthService);
  readonly authenticated = signal<boolean | null>(null);
  readonly passwordSet = signal(true);

  async refresh(): Promise<boolean> {
    try {
      const status = await firstValueFrom(this.api.authStatus());
      this.authenticated.set(status.authenticated);
      this.passwordSet.set(status.password_set);
      return status.authenticated;
    } catch {
      this.authenticated.set(false);
      return false;
    }
  }

  async login(password: string): Promise<void> {
    await firstValueFrom(this.api.login({ body: { password } }));
    this.authenticated.set(true);
  }

  async logout(everywhere = false): Promise<void> {
    await firstValueFrom(everywhere ? this.api.logoutEverywhere() : this.api.logout());
    this.authenticated.set(false);
  }

  markLoggedOut(): void {
    this.authenticated.set(false);
  }
}

/** Les écrans de l'application exigent une session ; sinon, retour à la connexion avec l'URL demandée. */
export const requireSession: CanActivateFn = async (_route, state) => {
  const session = inject(Session);
  const router = inject(Router);
  const ok = session.authenticated() ?? (await session.refresh());
  return ok ? true : router.createUrlTree(['/connexion'], { queryParams: { retour: state.url } });
};
