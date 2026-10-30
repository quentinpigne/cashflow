import { ChangeDetectionStrategy, Component, DestroyRef, inject, signal } from '@angular/core';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { Subject, catchError, debounceTime, of, switchMap } from 'rxjs';

import { CreditConditions } from '../../api/models/credit-conditions';
import { ProjectDefaults } from '../../api/models/project-defaults';
import { ProjectKind } from '../../api/models/project-kind';
import { CreditService } from '../../api/services/credit.service';
import { errorMessage } from '../../core/http-errors';
import { ConditionsFields } from './conditions';
import { PROJECT_KIND_LABELS } from './credit-shared';

/** E42 — Conditions de crédit par défaut de chaque type de projet (CRE-12, R-CRE-38). */
@Component({
  selector: 'app-credit-defaults',
  imports: [RouterLink, MatButtonModule, MatIconModule, MatProgressBarModule, ConditionsFields],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/reglages" aria-label="Retour aux réglages">
          <mat-icon>arrow_back</mat-icon>
        </a>
        <h1>Conditions de crédit</h1>
      </div>
      <p class="muted">
        Les conditions proposées pour chaque type de projet dans l'onglet Simuler. Des ordres de
        grandeur : ajustez-les aux offres du moment. Chaque changement est gardé.
      </p>
      @for (d of defaults(); track d.project_kind) {
        <section class="card" [attr.aria-label]="labels[d.project_kind]">
          <header>
            <h2>{{ labels[d.project_kind] }}</h2>
            <button
              mat-button
              [disabled]="same(d.conditions, d.initial)"
              (click)="change(d.project_kind, d.initial)"
            >
              <mat-icon>restart_alt</mat-icon>Valeurs initiales
            </button>
          </header>
          <app-conditions-fields
            [value]="d.conditions"
            (changed)="change(d.project_kind, $event)"
          />
        </section>
      } @empty {
        <mat-progress-bar mode="indeterminate" />
      }
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 0.5rem;
      margin-bottom: 0.75rem;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
    }
  `,
})
export class CreditDefaultsPage {
  private readonly api = inject(CreditService);
  private readonly snack = inject(MatSnackBar);

  protected readonly labels = PROJECT_KIND_LABELS;
  protected readonly defaults = signal<ProjectDefaults[]>([]);
  private readonly saves = new Subject<ProjectDefaults[]>();

  constructor() {
    this.api.projectDefaults().subscribe((d) => this.defaults.set(d));
    this.saves
      .pipe(
        debounceTime(400),
        switchMap((all) =>
          this.api
            .saveProjectDefaults({
              body: all.map((d) => ({ project_kind: d.project_kind, conditions: d.conditions })),
            })
            .pipe(
              catchError((e) => {
                this.snack.open(errorMessage(e), undefined, { duration: 5000 });
                return of(null);
              }),
            ),
        ),
        takeUntilDestroyed(inject(DestroyRef)),
      )
      .subscribe();
  }

  protected same(a: CreditConditions, b: CreditConditions): boolean {
    return JSON.stringify(a) === JSON.stringify(b);
  }

  protected change(project: ProjectKind, conditions: CreditConditions): void {
    this.defaults.update((all) =>
      all.map((d) => (d.project_kind === project ? { ...d, conditions } : d)),
    );
    this.saves.next(this.defaults());
  }
}
