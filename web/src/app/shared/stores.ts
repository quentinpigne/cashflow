import { Injectable, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { firstValueFrom } from 'rxjs';

import { Account } from '../api/models/account';
import { Category } from '../api/models/category';
import { Institution } from '../api/models/institution';
import { PayeeGroupView } from '../api/models/payee-group-view';
import { PayeeView } from '../api/models/payee-view';
import { ProjectView } from '../api/models/project-view';
import { AccountsService } from '../api/services/accounts.service';
import { BudgetsService } from '../api/services/budgets.service';
import { CategoriesService } from '../api/services/categories.service';
import { InstitutionsService } from '../api/services/institutions.service';
import { AssetsService } from '../api/services/assets.service';
import { LoansService } from '../api/services/loans.service';
import { PayeesService } from '../api/services/payees.service';
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

/** Tiers et groupes de tiers (spec 11), partagés par tous les écrans. */
@Injectable({ providedIn: 'root' })
export class PayeesStore {
  private readonly api = inject(PayeesService);
  private readonly version = inject(DataVersion);
  /** Noms des tiers créés à la volée, connus avant le rechargement de la liste (R-TIE-15). */
  private readonly created = signal(new Map<string, string>());

  readonly all = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listPayees(),
  });

  readonly list = computed<PayeeView[]>(() => (this.all.hasValue() ? this.all.value().payees : []));
  readonly groups = computed<PayeeGroupView[]>(() =>
    this.all.hasValue() ? this.all.value().groups : [],
  );
  readonly byId = computed(() => new Map(this.list().map((p) => [p.id, p])));
  readonly groupById = computed(() => new Map(this.groups().map((g) => [g.id, g])));

  /** Nom d'un tiers ; vide sans tiers. */
  name(id: string | null | undefined): string {
    if (!id) return '';
    return this.byId().get(id)?.name ?? this.created().get(id) ?? '—';
  }

  groupName(id: string | null | undefined): string {
    return id ? (this.groupById().get(id)?.name ?? '—') : '';
  }

  /** Crée un tiers sans groupe depuis la saisie (R-TIE-15) et retourne son identifiant. */
  async create(name: string): Promise<string> {
    const created = await firstValueFrom(
      this.api.createPayee({ body: { name, group_id: null, archived: false, note: null } }),
    );
    this.created.update((m) => new Map(m).set(created.id, name.trim()));
    this.version.bump();
    return created.id;
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

/**
 * Rubriques activées (R-BIE-7, R-PRE-12, R-SAL-14) et recherche d'adresse (R-TIE-26), pour le menu, l'accueil et
 * les formulaires.
 */
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
  readonly salaries = computed(() => this.features.value()?.salaries ?? false);
  readonly geocoding = computed(() => this.features.value()?.geocoding ?? false);
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
