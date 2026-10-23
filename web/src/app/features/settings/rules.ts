import { CdkDragDrop, DragDropModule, moveItemInArray } from '@angular/cdk/drag-drop';
import { ChangeDetectionStrategy, Component, OnInit, inject, input } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { RuleInput } from '../../api/models/rule-input';
import { RuleView } from '../../api/models/rule-view';
import { RulesService } from '../../api/services/rules.service';
import { TransactionsService } from '../../api/services/transactions.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { abs } from '../../core/money';
import { confirm } from '../../shared/confirm-dialog';
import { CategoriesStore, PayeesStore } from '../../shared/stores';
import { RuleFormData, RuleFormDialog } from './rule-form';

/** E16 — Règles de catégorisation (IMP-08). */
@Component({
  selector: 'app-rules',
  imports: [RouterLink, DragDropModule, MatButtonModule, MatIconModule, MatSlideToggleModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/reglages" aria-label="Retour aux réglages"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Règles</h1>
        <button mat-flat-button (click)="edit(null)"><mat-icon>add</mat-icon>Ajouter</button>
      </div>
      <p class="muted">
        Évaluées dans l'ordre, à l'import et à la demande. Glisser pour changer la priorité.
      </p>
      <div class="card" cdkDropList (cdkDropListDropped)="drop($event)">
        @for (rule of rules.value() ?? []; track rule.id) {
          <div class="row" cdkDrag>
            <mat-icon class="handle" cdkDragHandle aria-hidden="true">drag_indicator</mat-icon>
            <button class="main" (click)="edit(rule)">
              <span>{{ rule.name }}</span>
              <span class="muted small"
                >{{ summary(rule) }} · {{ rule.hit_count }} utilisation(s)</span
              >
            </button>
            <mat-slide-toggle
              [checked]="rule.active"
              (change)="setActive(rule, $event.checked)"
              [attr.aria-label]="'Activer ' + rule.name"
            />
            <button
              mat-icon-button
              (click)="remove(rule)"
              [attr.aria-label]="'Supprimer ' + rule.name"
            >
              <mat-icon>delete</mat-icon>
            </button>
          </div>
        } @empty {
          <p class="empty">
            Aucune règle. Depuis une transaction, « Créer une règle à partir de celle-ci » en
            prépare une.
          </p>
        }
      </div>
    </div>
  `,
  styles: `
    .row {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      background: var(--mat-sys-surface-container-low);
    }
    .main {
      flex: 1;
      display: flex;
      flex-direction: column;
      padding: 0.5rem 0;
      border: none;
      background: none;
      color: inherit;
      font: inherit;
      text-align: left;
      cursor: pointer;
    }
    .handle {
      cursor: grab;
      color: var(--mat-sys-on-surface-variant);
    }
    .small {
      font: var(--mat-sys-body-small);
    }
  `,
})
export class RulesPage implements OnInit {
  private readonly api = inject(RulesService);
  private readonly transactions = inject(TransactionsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly categories = inject(CategoriesStore);
  private readonly payees = inject(PayeesStore);

  /** Transaction de départ (R-IMP-20). */
  readonly depuis = input<string>();

  protected readonly rules = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listRules(),
  });

  async ngOnInit(): Promise<void> {
    const id = this.depuis();
    if (!id) {
      return;
    }
    // Préremplit la condition sur le libellé et l'action sur la ventilation de la transaction.
    const t = await firstValueFrom(this.transactions.getTransaction({ id }));
    const label = (t.bank_label ?? t.label).replace(/\s+\d{2}\/\d{2}.*$/, '').trim();
    const draft: Partial<RuleInput> = {
      name: t.label,
      conditions: {
        label: { op: 'contains', value: label, field: t.bank_label ? 'bank_label' : 'label' },
        accounts: [],
      },
      actions: {
        set_splits: t.splits
          .filter((s) => s.category_id)
          .map((s) => ({ category_id: s.category_id, weight: abs(s.amount_minor) })),
        set_label: t.bank_label && t.label !== t.bank_label ? t.label : null,
        set_payee: t.payee_id ?? null,
      },
    };
    this.open({ rule: null, draft });
  }

  protected summary(rule: RuleView): string {
    const parts: string[] = [];
    const label = rule.conditions.label;
    if (label)
      parts.push(
        `libellé ${label.op === 'contains' ? 'contient' : label.op === 'starts_with' ? 'commence par' : label.op === 'equals' ? '=' : '≈'} « ${label.value} »`,
      );
    if (rule.actions.mark_transfer_to) parts.push('→ virement');
    else if (rule.actions.set_splits?.length)
      parts.push(
        `→ ${rule.actions.set_splits.map((s) => this.categories.label(s.category_id)).join(', ')}`,
      );
    if (rule.actions.set_payee && !rule.actions.mark_transfer_to)
      parts.push(`→ tiers ${this.payees.name(rule.actions.set_payee)}`);
    return parts.join(' ');
  }

  protected edit(rule: RuleView | null): void {
    this.open({ rule });
  }

  private async open(data: RuleFormData): Promise<void> {
    const ref = this.dialog.open(RuleFormDialog, { data, width: '44rem', maxWidth: '100vw' });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  private input(rule: RuleView, change: Partial<RuleInput>): RuleInput {
    return {
      name: rule.name,
      priority: rule.priority,
      conditions: rule.conditions,
      actions: rule.actions,
      stop_processing: rule.stop_processing,
      active: rule.active,
      ...change,
    };
  }

  protected async setActive(rule: RuleView, active: boolean): Promise<void> {
    await this.run(() =>
      firstValueFrom(this.api.updateRule({ id: rule.id, body: this.input(rule, { active }) })),
    );
  }

  protected async remove(rule: RuleView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${rule.name} »`,
      message: 'Les transactions déjà catégorisées ne changent pas.',
      action: 'Supprimer',
      destructive: true,
    });
    if (ok) {
      await this.run(() => firstValueFrom(this.api.deleteRule({ id: rule.id })));
    }
  }

  /** Nouvelle priorité : l'ordre de la liste. */
  protected async drop(event: CdkDragDrop<RuleView[]>): Promise<void> {
    const list = [...(this.rules.value() ?? [])];
    moveItemInArray(list, event.previousIndex, event.currentIndex);
    await this.run(async () => {
      for (const [index, rule] of list.entries()) {
        if (rule.priority !== index) {
          await firstValueFrom(
            this.api.updateRule({ id: rule.id, body: this.input(rule, { priority: index }) }),
          );
        }
      }
    });
  }

  private async run(action: () => Promise<unknown>): Promise<void> {
    try {
      await action();
      this.version.bump();
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }
}
