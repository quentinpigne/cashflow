import { Injectable, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';

import { Account } from '../api/models/account';
import { Category } from '../api/models/category';
import { Institution } from '../api/models/institution';
import { AccountsService } from '../api/services/accounts.service';
import { CategoriesService } from '../api/services/categories.service';
import { InstitutionsService } from '../api/services/institutions.service';
import { DataVersion } from '../core/data-version';

/** Une catégorie à plat, avec son libellé complet « Parent › Enfant ». */
export interface FlatCategory {
  id: string;
  name: string;
  label: string;
  kind: Category['kind'];
  parentId: string | null;
  parentName: string | null;
  archived: boolean;
}

/** Catégories partagées par tous les écrans, rechargées à chaque modification. */
@Injectable({ providedIn: 'root' })
export class CategoriesStore {
  private readonly api = inject(CategoriesService);
  private readonly version = inject(DataVersion);

  readonly tree = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listCategories(),
  });

  readonly flat = computed<FlatCategory[]>(() => {
    const roots = this.tree.hasValue() ? this.tree.value() : [];
    return roots.flatMap((root) => [
      {
        id: root.id,
        name: root.name,
        label: root.name,
        kind: root.kind,
        parentId: null,
        parentName: null,
        archived: root.archived,
      },
      ...root.children.map((child) => ({
        id: child.id,
        name: child.name,
        label: `${root.name} › ${child.name}`,
        kind: child.kind,
        parentId: root.id,
        parentName: root.name,
        archived: child.archived || root.archived,
      })),
    ]);
  });

  readonly byId = computed(() => new Map(this.flat().map((c) => [c.id, c])));

  label(id: string | null | undefined): string {
    return id ? (this.byId().get(id)?.label ?? '—') : 'Non catégorisé';
  }
}

/** Comptes avec leur solde du jour, partagés par tous les écrans. */
@Injectable({ providedIn: 'root' })
export class AccountsStore {
  private readonly api = inject(AccountsService);
  private readonly version = inject(DataVersion);

  readonly all = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listAccounts(),
  });

  readonly list = computed<Account[]>(() => (this.all.hasValue() ? this.all.value() : []));
  /** Comptes proposés en saisie : les clôturés n'y apparaissent plus (CPT-04). */
  readonly open = computed(() => this.list().filter((a) => !a.closed_on));
  readonly byId = computed(() => new Map(this.list().map((a) => [a.id, a])));
}

@Injectable({ providedIn: 'root' })
export class InstitutionsStore {
  private readonly api = inject(InstitutionsService);
  private readonly version = inject(DataVersion);

  readonly all = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listInstitutions(),
  });
  readonly list = computed<Institution[]>(() => (this.all.hasValue() ? this.all.value() : []));
  /** Établissements proposés à la création de compte (R-CPT-10). */
  readonly active = computed(() => this.list().filter((i) => !i.archived));
}
