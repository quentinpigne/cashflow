import { Injectable, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';

import { Account } from '../api/models/account';
import { Category } from '../api/models/category';
import { Institution } from '../api/models/institution';
import { ProjectView } from '../api/models/project-view';
import { AccountsService } from '../api/services/accounts.service';
import { BudgetsService } from '../api/services/budgets.service';
import { CategoriesService } from '../api/services/categories.service';
import { InstitutionsService } from '../api/services/institutions.service';
import { AssetsService } from '../api/services/assets.service';
import { LoansService } from '../api/services/loans.service';
import { SystemService } from '../api/services/system.service';
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

/** Projets (BUD-06), partagés par les écrans qui y rattachent des transactions. */
@Injectable({ providedIn: 'root' })
export class ProjectsStore {
  private readonly api = inject(BudgetsService);
  private readonly version = inject(DataVersion);

  readonly all = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listProjects(),
  });
  readonly list = computed<ProjectView[]>(() => (this.all.hasValue() ? this.all.value() : []));
  /** On ne rattache de nouvelles transactions qu'à un projet en cours (R-BUD-24). */
  readonly active = computed(() => this.list().filter((p) => p.active));
  readonly byId = computed(() => new Map(this.list().map((p) => [p.id, p])));
}

/** Rubriques activées (R-BIE-7, R-PRE-12), pour le menu, l'accueil et les formulaires. */
@Injectable({ providedIn: 'root' })
export class FeaturesStore {
  private readonly api = inject(SystemService);
  private readonly version = inject(DataVersion);

  readonly features = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.getFeatures(),
  });
  readonly assets = computed(() => this.features.value()?.assets ?? false);
  readonly loans = computed(() => this.features.value()?.loans ?? false);
}

/** Un rattachement possible d'une ventilation, à la place d'une catégorie (R-TRX-37). */
export interface AttachmentOption {
  /** `loan:<id>` ou `asset:<id>`. */
  value: string;
  group: 'Remboursement' | 'Acquisition';
  name: string;
  label: string;
}

/**
 * Prêts et biens auxquels rattacher une ventilation : remboursement (R-PRE-7) et acquisition (R-BIE-8), seulement
 * si la rubrique est activée.
 */
@Injectable({ providedIn: 'root' })
export class AttachmentsStore {
  private readonly loansApi = inject(LoansService);
  private readonly assetsApi = inject(AssetsService);
  private readonly version = inject(DataVersion);
  private readonly features = inject(FeaturesStore);

  private readonly loans = rxResource({
    params: () => (this.features.loans() ? this.version.value() : undefined),
    stream: () => this.loansApi.listLoans(),
  });
  private readonly assets = rxResource({
    params: () => (this.features.assets() ? this.version.value() : undefined),
    stream: () => this.assetsApi.listAssets(),
  });

  readonly options = computed<AttachmentOption[]>(() => [
    ...(this.features.loans() ? (this.loans.value() ?? []) : [])
      .filter((l) => !l.closed_on)
      .map((l) => ({
        value: `loan:${l.id}`,
        group: 'Remboursement' as const,
        name: l.name,
        label: `Remboursement › ${l.name}`,
      })),
    ...(this.features.assets() ? (this.assets.value() ?? []) : [])
      .filter((a) => !a.sold_on)
      .map((a) => ({
        value: `asset:${a.id}`,
        group: 'Acquisition' as const,
        name: a.name,
        label: `Acquisition › ${a.name}`,
      })),
  ]);
  readonly byValue = computed(() => new Map(this.options().map((o) => [o.value, o])));
}
