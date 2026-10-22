import { ChangeDetectionStrategy, Component, OnInit, inject, input, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { Router } from '@angular/router';

import { Session } from '../../core/auth';
import { errorMessage, problemOf } from '../../core/http-errors';

/** E0 — Connexion : un seul champ, le mot de passe (ADR-0015). */
@Component({
  selector: 'app-login',
  imports: [FormsModule, MatFormFieldModule, MatInputModule, MatButtonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <main class="login">
      <h1>Cashflow</h1>
      @if (!session.passwordSet()) {
        <p class="notice">
          Aucun mot de passe n'est défini. Sur le serveur, lancer
          <code>cashflow set-password</code>.
        </p>
      } @else {
        <form (ngSubmit)="submit()">
          <mat-form-field class="full-width" subscriptSizing="dynamic">
            <mat-label>Mot de passe</mat-label>
            <input
              matInput
              type="password"
              name="password"
              autocomplete="current-password"
              [ngModel]="password()"
              (ngModelChange)="password.set($event)"
              required
            />
            @if (error()) {
              <mat-hint class="error">{{ error() }}</mat-hint>
            }
          </mat-form-field>
          <button
            mat-flat-button
            class="full-width"
            type="submit"
            [disabled]="busy() || !password()"
          >
            Se connecter
          </button>
        </form>
      }
    </main>
  `,
  styles: `
    .login {
      max-width: 22rem;
      margin: 0 auto;
      padding: 15vh 1.5rem 2rem;
    }
    h1 {
      font: var(--mat-sys-display-small);
      color: var(--mat-sys-primary);
      margin: 0 0 2rem;
    }
    form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
    }
    .error {
      color: var(--mat-sys-error);
    }
    .notice {
      background: var(--mat-sys-surface-container);
      padding: 1rem;
      border-radius: var(--mat-sys-corner-medium);
    }
  `,
})
export class LoginPage implements OnInit {
  protected readonly session = inject(Session);
  private readonly router = inject(Router);

  /** URL demandée avant la redirection vers la connexion. */
  readonly retour = input<string>();

  protected readonly password = signal('');
  protected readonly error = signal<string | null>(null);
  protected readonly busy = signal(false);

  async ngOnInit(): Promise<void> {
    if (await this.session.refresh()) {
      await this.router.navigateByUrl(this.retour() ?? '/');
    }
  }

  protected async submit(): Promise<void> {
    this.busy.set(true);
    this.error.set(null);
    try {
      await this.session.login(this.password());
      await this.router.navigateByUrl(this.retour() ?? '/');
    } catch (error) {
      const problem = problemOf(error);
      if (problem?.code === 'TOO_MANY_ATTEMPTS' && problem.retry_after) {
        this.error.set(`Trop de tentatives. Réessayer dans ${problem.retry_after} s.`);
      } else if (problem?.code === 'NO_PASSWORD') {
        this.session.passwordSet.set(false);
      } else {
        this.error.set(
          problem?.code === 'WRONG_PASSWORD' ? 'Mot de passe incorrect.' : errorMessage(error),
        );
      }
    } finally {
      this.busy.set(false);
    }
  }
}
