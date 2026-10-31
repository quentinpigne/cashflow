import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { firstValueFrom } from 'rxjs';

import { AssetShareInput } from '../../api/models/asset-share-input';
import { AssetView } from '../../api/models/asset-view';
import { AssetsService } from '../../api/services/assets.service';
import { errorMessage } from '../../core/http-errors';
import {
  formatMoney,
  formatShare,
  parseMoney,
  parsePercent,
  shareOfAmount,
} from '../../core/money';
import { MembersStore } from '../../shared/stores';

interface Line {
  memberId: string;
  name: string;
  /** Saisie en pourcentage ou en euros, selon le mode. */
  input: string;
}

/** Répartition d'un bien entre membres (MBR-06, R-MBR-11) : quote-part en % ou en €, le reste au foyer. */
@Component({
  selector: 'app-asset-shares-dialog',
  imports: [
    FormsModule,
    MatButtonModule,
    MatButtonToggleModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Répartition</h2>
    <mat-dialog-content class="form">
      <p class="muted">
        Valeur actuelle : {{ value() === null ? '—' : format(value()!) }}. Ce qui n'est pas réparti
        reste au foyer.
      </p>
      <mat-button-toggle-group
        [value]="mode()"
        (change)="switchMode($event.value)"
        aria-label="Saisie des quotes-parts"
      >
        <mat-button-toggle value="percent">%</mat-button-toggle>
        <mat-button-toggle value="amount" [disabled]="value() === null">€</mat-button-toggle>
      </mat-button-toggle-group>
      @for (line of lines(); track line.memberId; let i = $index) {
        <mat-form-field>
          <mat-label>{{ line.name }}</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="line.input"
            (ngModelChange)="update(i, $event)"
            [name]="'share' + i"
          />
          <span matTextSuffix>{{ mode() === 'percent' ? '%' : '€' }}</span>
          @if (mode() === 'amount' && shares()[i]; as bp) {
            <mat-hint>{{ share(bp) }}</mat-hint>
          }
        </mat-form-field>
      }
      <p [class.error]="household() === null || household()! < 0">
        @if (household() === null) {
          Saisie invalide.
        } @else if (household()! < 0) {
          La répartition dépasse 100 %.
        } @else {
          Foyer : {{ share(household()!) }}
        }
      </p>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!valid() || busy()" (click)="save()">Enregistrer</button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(22rem, 80vw);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class AssetSharesDialog {
  private readonly asset = inject<AssetView>(MAT_DIALOG_DATA);
  private readonly api = inject(AssetsService);
  private readonly ref = inject(MatDialogRef<AssetSharesDialog>);
  private readonly members = inject(MembersStore);

  protected readonly value = signal<number | null>(this.asset.summary.value_minor ?? null);
  protected readonly mode = signal<'percent' | 'amount'>('percent');
  protected readonly lines = signal<Line[]>(
    this.members.list().map((m) => {
      const bp = this.asset.summary.shares.find((s) => s.member_id === m.id)?.share_bp;
      return { memberId: m.id, name: m.name, input: bp ? this.editableShare(bp) : '' };
    }),
  );
  protected readonly error = signal<string | null>(null);
  protected readonly busy = signal(false);

  /** Quote-part de chaque ligne en centièmes de pour cent ; 0 si vide, `null` si invalide. */
  protected readonly shares = computed<(number | null)[]>(() =>
    this.lines().map((l) => {
      if (!l.input.trim()) return 0;
      if (this.mode() === 'percent') return parsePercent(l.input);
      const amount = parseMoney(l.input);
      const value = this.value();
      return amount === null || value === null ? null : shareOfAmount(amount, value);
    }),
  );

  /** Part du foyer : le reste ; `null` si une saisie est invalide. */
  protected readonly household = computed<number | null>(() => {
    const shares = this.shares();
    if (shares.some((s) => s === null)) return null;
    return 10_000 - (shares as number[]).reduce((a, b) => a + b, 0);
  });

  protected readonly valid = computed(() => {
    const household = this.household();
    return (
      household !== null && household >= 0 && this.shares().every((s) => s === null || s <= 10_000)
    );
  });

  protected share(bp: number): string {
    return formatShare(bp);
  }

  protected format(minor: number): string {
    return formatMoney(minor);
  }

  private editableShare(bp: number): string {
    return formatShare(bp).replace(/\u00a0%$/, '');
  }

  protected update(index: number, input: string): void {
    this.lines.update((lines) => lines.map((l, i) => (i === index ? { ...l, input } : l)));
  }

  /** Changer de mode reprend les quotes-parts en pourcentage, ou efface la saisie en euros. */
  protected switchMode(mode: 'percent' | 'amount'): void {
    if (mode === this.mode()) return;
    const shares = this.shares();
    this.lines.update((lines) =>
      lines.map((l, i) => ({
        ...l,
        input: mode === 'percent' && shares[i] ? this.editableShare(shares[i] as number) : '',
      })),
    );
    this.mode.set(mode);
  }

  protected async save(): Promise<void> {
    const shares: AssetShareInput[] = [];
    this.lines().forEach((l, i) => {
      if (!l.input.trim()) return;
      if (this.mode() === 'percent') {
        shares.push({ member_id: l.memberId, share_bp: this.shares()[i], amount_minor: null });
      } else {
        shares.push({ member_id: l.memberId, share_bp: null, amount_minor: parseMoney(l.input) });
      }
    });
    this.busy.set(true);
    try {
      await firstValueFrom(
        this.api.setAssetShares({ id: this.asset.summary.id, body: { shares } }),
      );
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    } finally {
      this.busy.set(false);
    }
  }
}
