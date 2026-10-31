import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatRadioModule } from '@angular/material/radio';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { AcceptSuggestionInput } from '../../api/models/accept-suggestion-input';
import { PayeeSuggestionView } from '../../api/models/payee-suggestion-view';
import { PayeesService } from '../../api/services/payees.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { FrDatePipe } from '../../shared/date.pipe';
import { PayeePicker } from '../../shared/payee-picker';

/** Validation d'une proposition (R-TIE-33, R-TIE-34) : un tiers à créer ou un tiers existant, et la règle. */
@Component({
  selector: 'app-accept-payee-suggestion',
  imports: [
    FormsModule,
    MatButtonModule,
    MatCheckboxModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
    MatRadioModule,
    PayeePicker,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Valider la proposition</h2>
    <mat-dialog-content class="form">
      <mat-radio-group
        class="modes"
        [ngModel]="mode()"
        (ngModelChange)="mode.set($event)"
        aria-label="Tiers à affecter"
      >
        <mat-radio-button value="create">Créer le tiers</mat-radio-button>
        <mat-radio-button value="existing">Affecter à un tiers existant</mat-radio-button>
      </mat-radio-group>
      @if (mode() === 'create') {
        <mat-form-field>
          <mat-label>Nom du tiers</mat-label>
          <input
            matInput
            [ngModel]="name()"
            (ngModelChange)="name.set($event)"
            maxlength="80"
            cdkFocusInitial
          />
        </mat-form-field>
      } @else {
        <app-payee-picker
          [value]="payeeId()"
          (valueChange)="payeeId.set($event)"
          [creatable]="false"
          subscript="fixed"
        />
      }
      <mat-checkbox [ngModel]="createRule()" (ngModelChange)="createRule.set($event)">
        Créer aussi une règle pour les prochains imports
      </mat-checkbox>
      <p class="muted">
        {{ data.transaction_count }} transactions recevront ce tiers. Leurs catégories ne changent
        pas.
      </p>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [mat-dialog-close]="result()" [disabled]="!result()">Valider</button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(22rem, 80vw);
    }

    .modes {
      display: flex;
      flex-direction: column;
    }
  `,
})
export class AcceptSuggestionDialog {
  protected readonly data = inject<PayeeSuggestionView>(MAT_DIALOG_DATA);
  protected readonly mode = signal<'create' | 'existing'>(
    this.data.payee_id ? 'existing' : 'create',
  );
  protected readonly name = signal(this.data.name);
  protected readonly payeeId = signal<string | null>(this.data.payee_id ?? null);
  protected readonly createRule = signal(true);

  protected readonly result = computed<AcceptSuggestionInput | undefined>(() => {
    const base = { key: this.data.key, create_rule: this.createRule() };
    if (this.mode() === 'existing') {
      const payee = this.payeeId();
      return payee ? { ...base, payee_id: payee, name: null } : undefined;
    }
    const name = this.name().trim();
    return name ? { ...base, payee_id: null, name } : undefined;
  });
}

/** E43 — Propositions de tiers à partir des libellés bancaires (TIE-05). */
@Component({
  selector: 'app-payee-suggestions',
  imports: [
    RouterLink,
    MatButtonModule,
    MatExpansionModule,
    MatIconModule,
    MatProgressBarModule,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/reglages/tiers" aria-label="Retour aux tiers"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Propositions de tiers</h1>
      </div>

      @if (list.isLoading() && !list.hasValue()) {
        <mat-progress-bar mode="indeterminate" />
      }

      @for (s of suggestions(); track s.key) {
        <section class="card suggestion">
          <div class="head">
            <h2>
              @if (s.payee_name) {
                <mat-icon class="arrow">arrow_forward</mat-icon>{{ s.payee_name }}
              } @else {
                {{ s.name }}
              }
            </h2>
            <span class="muted"
              >{{ s.transaction_count }} transactions, du {{ s.first_date | frDate }} au
              {{ s.last_date | frDate }}</span
            >
          </div>
          <ul class="labels">
            @for (label of s.labels; track label) {
              <li>{{ label }}</li>
            }
          </ul>
          <div class="actions">
            <button mat-button (click)="ignore(s)">Ignorer</button>
            <button mat-flat-button (click)="accept(s)">Valider…</button>
          </div>
        </section>
      } @empty {
        @if (list.hasValue()) {
          <p class="empty">
            Aucune proposition : les transactions sans tiers n'ont pas de libellés qui se répètent.
          </p>
        }
      }

      @if (ignored().length > 0) {
        <mat-accordion>
          <mat-expansion-panel class="ignored">
            <mat-expansion-panel-header>
              <mat-panel-title>Clés ignorées ({{ ignored().length }})</mat-panel-title>
            </mat-expansion-panel-header>
            @for (i of ignored(); track i.key) {
              <div class="row">
                <span class="key">{{ i.key }}</span>
                <button mat-button (click)="restore(i.key)">Rétablir</button>
              </div>
            }
          </mat-expansion-panel>
        </mat-accordion>
      }
    </div>
  `,
  styles: `
    .suggestion {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      margin-bottom: 0.75rem;

      .head {
        display: flex;
        flex-wrap: wrap;
        align-items: baseline;
        column-gap: 1rem;
      }

      h2 {
        display: flex;
        align-items: center;
        margin: 0;
        font: var(--mat-sys-title-medium);
      }

      .arrow {
        margin-right: 0.25rem;
        color: var(--mat-sys-primary);
      }

      .muted {
        font: var(--mat-sys-body-small);
      }
    }

    .labels {
      margin: 0;
      padding: 0;
      list-style: none;

      li {
        overflow: hidden;
        color: var(--mat-sys-on-surface-variant);
        font-family: monospace;
        font-size: 0.85rem;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
    }

    .actions {
      display: flex;
      justify-content: flex-end;
      gap: 0.5rem;
    }

    .empty {
      padding: 0.5rem;
    }

    .ignored {
      margin-top: 1rem;
    }

    .row {
      display: flex;
      align-items: center;
      min-height: 2.75rem;
      border-bottom: 1px solid var(--mat-sys-outline-variant);

      .key {
        flex: 1;
        font-family: monospace;
      }
    }
  `,
})
export class PayeeSuggestionsPage {
  private readonly api = inject(PayeesService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  protected readonly list = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listPayeeSuggestions(),
  });

  protected readonly suggestions = computed(() =>
    this.list.hasValue() ? this.list.value().suggestions : [],
  );
  protected readonly ignored = computed(() =>
    this.list.hasValue() ? this.list.value().ignored : [],
  );

  protected async accept(suggestion: PayeeSuggestionView): Promise<void> {
    const ref = this.dialog.open<
      AcceptSuggestionDialog,
      PayeeSuggestionView,
      AcceptSuggestionInput
    >(AcceptSuggestionDialog, { data: suggestion, width: '28rem', maxWidth: '100vw' });
    const input = await firstValueFrom(ref.afterClosed());
    if (!input) return;
    try {
      const result = await firstValueFrom(this.api.acceptPayeeSuggestion({ body: input }));
      this.version.bump();
      const rule = result.rule_id ? ' Règle créée.' : '';
      this.snack.open(`${result.assigned} transactions ont reçu ce tiers.${rule}`, undefined, {
        duration: 4000,
      });
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }

  protected async ignore(suggestion: PayeeSuggestionView): Promise<void> {
    try {
      await firstValueFrom(this.api.ignorePayeeKey({ body: { key: suggestion.key } }));
      this.version.bump();
      const undo = this.snack.open('Proposition ignorée', 'Annuler', { duration: 5000 });
      undo.onAction().subscribe(() => void this.restore(suggestion.key));
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }

  protected async restore(key: string): Promise<void> {
    try {
      await firstValueFrom(this.api.restorePayeeKey({ body: { key } }));
      this.version.bump();
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }
}
