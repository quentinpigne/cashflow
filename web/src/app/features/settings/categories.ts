import { CdkDragDrop, DragDropModule, moveItemInArray } from '@angular/cdk/drag-drop';
import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTabsModule } from '@angular/material/tabs';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { Category } from '../../api/models/category';
import { CategoryKind } from '../../api/models/category-kind';
import { CategoriesService } from '../../api/services/categories.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { confirm } from '../../shared/confirm-dialog';
import { CategoriesStore } from '../../shared/stores';
import { CategoryMergeDialog, CategoryNameDialog, NameData } from './category-dialogs';

/** E8 — Catégories (TRX-04). */
@Component({
  selector: 'app-categories',
  imports: [
    RouterLink,
    DragDropModule,
    MatButtonModule,
    MatExpansionModule,
    MatIconModule,
    MatMenuModule,
    MatProgressBarModule,
    MatTabsModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './categories.html',
  styleUrl: './categories.scss',
})
export class CategoriesPage {
  protected readonly store = inject(CategoriesStore);
  private readonly api = inject(CategoriesService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  protected readonly kind = signal<CategoryKind>('EXPENSE');
  private readonly roots = computed(() =>
    (this.store.tree.hasValue() ? this.store.tree.value() : []).filter(
      (c) => c.kind === this.kind(),
    ),
  );
  protected readonly active = computed(() => this.roots().filter((c) => !c.archived));
  protected readonly archived = computed(() => {
    const roots = this.roots();
    return [
      ...roots.filter((c) => c.archived),
      ...roots.flatMap((r) => r.children.filter((c) => c.archived && !r.archived)),
    ];
  });

  protected activeChildren(category: Category): Category[] {
    return category.children.filter((c) => !c.archived);
  }

  /** Supprimable : ni la catégorie ni ses sous-catégories n'ont jamais servi (R-TRX-22). */
  protected deletable(category: Category): boolean {
    return category.usage === 0 && category.children.every((c) => c.usage === 0);
  }

  private async name(data: NameData): Promise<string | undefined> {
    const ref = this.dialog.open<CategoryNameDialog, NameData, string>(CategoryNameDialog, {
      data,
      width: '24rem',
      maxWidth: '100vw',
    });
    return firstValueFrom(ref.afterClosed());
  }

  private async run(action: () => Promise<unknown>, success?: string): Promise<void> {
    try {
      await action();
      this.version.bump();
      if (success) {
        this.snack.open(success, undefined, { duration: 3000 });
      }
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }

  private body(
    category: Category,
    change: Partial<{
      name: string;
      archived: boolean;
      excluded_from_analyses: boolean;
      parent_id: string | null;
      display_order: number;
    }>,
  ) {
    return {
      name: change.name ?? category.name,
      kind: category.kind,
      parent_id: change.parent_id !== undefined ? change.parent_id : (category.parent_id ?? null),
      archived: change.archived ?? category.archived,
      excluded_from_analyses: change.excluded_from_analyses ?? category.excluded_from_analyses,
      display_order: change.display_order ?? category.display_order,
      color: category.color ?? null,
      icon: category.icon ?? null,
    };
  }

  protected async create(parent: Category | null): Promise<void> {
    const name = await this.name({
      title: parent ? `Nouvelle sous-catégorie de « ${parent.name} »` : 'Nouvelle catégorie',
      name: '',
      action: 'Créer',
    });
    if (name) {
      await this.run(() =>
        firstValueFrom(
          this.api.createCategory({
            body: { name, kind: this.kind(), parent_id: parent?.id ?? null, archived: false },
          }),
        ),
      );
    }
  }

  protected async rename(category: Category): Promise<void> {
    const name = await this.name({ title: 'Renommer', name: category.name, action: 'Renommer' });
    if (name && name !== category.name) {
      await this.run(() =>
        firstValueFrom(
          this.api.updateCategory({ id: category.id, body: this.body(category, { name }) }),
        ),
      );
    }
  }

  protected async setArchived(category: Category, archived: boolean): Promise<void> {
    await this.run(
      () =>
        firstValueFrom(
          this.api.updateCategory({ id: category.id, body: this.body(category, { archived }) }),
        ),
      archived
        ? `« ${category.name} » archivée : masquée en saisie, conservée dans l'historique.`
        : undefined,
    );
  }

  /** Hors analyses : ni revenu ni dépense, comme un virement (R-TRX-24). */
  protected async setExcluded(category: Category, excluded: boolean): Promise<void> {
    await this.run(
      () =>
        firstValueFrom(
          this.api.updateCategory({
            id: category.id,
            body: this.body(category, { excluded_from_analyses: excluded }),
          }),
        ),
      excluded
        ? `« ${category.name} » hors analyses : ni revenu ni dépense, plus « à catégoriser ».`
        : undefined,
    );
  }

  protected async merge(category: Category): Promise<void> {
    const ref = this.dialog.open<CategoryMergeDialog, Category, string>(CategoryMergeDialog, {
      data: category,
      width: '28rem',
      maxWidth: '100vw',
    });
    const target = await firstValueFrom(ref.afterClosed());
    if (target) {
      await this.run(async () => {
        const result = await firstValueFrom(
          this.api.mergeCategory({ id: category.id, body: { target_id: target } }),
        );
        this.snack.open(`${result.moved_splits} ventilations réaffectées.`, undefined, {
          duration: 4000,
        });
      });
    }
  }

  protected async remove(category: Category): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${category.name} »`,
      message: category.children.length
        ? 'La catégorie et ses sous-catégories, jamais utilisées, seront supprimées.'
        : 'Cette catégorie n’a jamais été utilisée.',
      action: 'Supprimer',
      destructive: true,
    });
    if (ok) {
      await this.run(() => firstValueFrom(this.api.deleteCategory({ id: category.id })));
    }
  }

  /** Glisser-déposer : nouvel ordre d'un niveau de l'arbre. */
  protected async drop(list: Category[], event: CdkDragDrop<Category[]>): Promise<void> {
    if (event.previousIndex === event.currentIndex) {
      return;
    }
    const ordered = [...list];
    moveItemInArray(ordered, event.previousIndex, event.currentIndex);
    await this.run(() =>
      firstValueFrom(this.api.reorderCategories({ body: { ids: ordered.map((c) => c.id) } })),
    );
  }
}
