import { Injectable, inject } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
import { MatSnackBar } from '@angular/material/snack-bar';
import { firstValueFrom } from 'rxjs';

import { PayeeGroupView } from '../../api/models/payee-group-view';
import { PayeeView } from '../../api/models/payee-view';
import { PayeesService } from '../../api/services/payees.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { confirm } from '../../shared/confirm-dialog';
import {
  PayeeFormData,
  PayeeFormDialog,
  PayeeFormValue,
  PayeeMergeDialog,
  PayeeRegroupDialog,
} from './payee-dialogs';

/**
 * Actions sur les tiers et les groupes, communes à la liste (E31) et à la fiche (E32). Chacune retourne `true`
 * si elle a abouti.
 */
@Injectable({ providedIn: 'root' })
export class PayeeActions {
  private readonly api = inject(PayeesService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  private async run(action: () => Promise<unknown>, success?: string): Promise<boolean> {
    try {
      await action();
      this.version.bump();
      if (success) this.snack.open(success, undefined, { duration: 4000 });
      return true;
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
      return false;
    }
  }

  private async form(data: PayeeFormData): Promise<PayeeFormValue | undefined> {
    const ref = this.dialog.open<PayeeFormDialog, PayeeFormData, PayeeFormValue>(PayeeFormDialog, {
      data,
      width: '28rem',
      maxWidth: '100vw',
    });
    return firstValueFrom(ref.afterClosed());
  }

  /** Groupe choisi, créé au passage si un nouveau nom a été saisi. */
  private async groupOf(value: PayeeFormValue): Promise<string | null> {
    if (!value.newGroup) return value.groupId;
    const created = await firstValueFrom(
      this.api.createPayeeGroup({ body: { name: value.newGroup, note: null } }),
    );
    return created.id;
  }

  async createPayee(groupId: string | null): Promise<boolean> {
    const value = await this.form({
      title: 'Nouveau tiers',
      action: 'Créer',
      kind: 'payee',
      value: { groupId },
    });
    if (!value) return false;
    return this.run(async () => {
      const group = await this.groupOf(value);
      await firstValueFrom(
        this.api.createPayee({
          body: { name: value.name, group_id: group, archived: false, note: value.note },
        }),
      );
    });
  }

  /** Renomme, change de groupe ou d'annotation (R-TIE-3). */
  async editPayee(payee: PayeeView): Promise<boolean> {
    const value = await this.form({
      title: `Modifier « ${payee.name} »`,
      action: 'Enregistrer',
      kind: 'payee',
      value: { name: payee.name, groupId: payee.group_id ?? null, note: payee.note ?? null },
    });
    if (!value) return false;
    return this.run(async () => {
      const group = await this.groupOf(value);
      await firstValueFrom(
        this.api.updatePayee({
          id: payee.id,
          body: { name: value.name, group_id: group, archived: payee.archived, note: value.note },
        }),
      );
    });
  }

  /** Un tiers archivé n'est plus proposé en saisie, mais reste partout ailleurs (R-TIE-6). */
  setArchived(payee: PayeeView, archived: boolean): Promise<boolean> {
    return this.run(
      () =>
        firstValueFrom(
          this.api.updatePayee({
            id: payee.id,
            body: {
              name: payee.name,
              group_id: payee.group_id ?? null,
              archived,
              note: payee.note ?? null,
            },
          }),
        ),
      archived
        ? `« ${payee.name} » archivé : plus proposé en saisie, conservé partout.`
        : undefined,
    );
  }

  /** Fusion dans un autre tiers (R-TIE-7) ; retourne la cible, ou `null` si rien n'a été fait. */
  async merge(payee: PayeeView): Promise<string | null> {
    const ref = this.dialog.open<PayeeMergeDialog, PayeeView, string>(PayeeMergeDialog, {
      data: payee,
      width: '28rem',
      maxWidth: '100vw',
    });
    const target = await firstValueFrom(ref.afterClosed());
    if (!target) return null;
    let moved = 0;
    const done = await this.run(async () => {
      const result = await firstValueFrom(
        this.api.mergePayee({ id: payee.id, body: { target_id: target } }),
      );
      moved = result.moved_transactions;
    });
    if (!done) return null;
    this.snack.open(`${moved} transaction(s) réaffectée(s).`, undefined, { duration: 4000 });
    return target;
  }

  /** Seul un tiers jamais utilisé se supprime (R-TIE-5). */
  async remove(payee: PayeeView): Promise<boolean> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${payee.name} »`,
      message: 'Ce tiers n’a jamais été utilisé.',
      action: 'Supprimer',
      destructive: true,
    });
    return ok ? this.run(() => firstValueFrom(this.api.deletePayee({ id: payee.id }))) : false;
  }

  async regroup(ids: string[]): Promise<boolean> {
    const ref = this.dialog.open<PayeeRegroupDialog, number, string>(PayeeRegroupDialog, {
      data: ids.length,
      width: '24rem',
      maxWidth: '100vw',
    });
    const group = await firstValueFrom(ref.afterClosed());
    if (!group) return false;
    return this.run(() =>
      firstValueFrom(
        this.api.regroupPayees({
          body: { payee_ids: ids, group_id: group === 'none' ? null : group },
        }),
      ),
    );
  }

  async createGroup(): Promise<boolean> {
    const value = await this.form({
      title: 'Nouveau groupe de tiers',
      action: 'Créer',
      kind: 'group',
      value: {},
    });
    if (!value) return false;
    return this.run(() =>
      firstValueFrom(this.api.createPayeeGroup({ body: { name: value.name, note: value.note } })),
    );
  }

  async editGroup(group: PayeeGroupView): Promise<boolean> {
    const value = await this.form({
      title: `Renommer « ${group.name} »`,
      action: 'Enregistrer',
      kind: 'group',
      value: { name: group.name, note: group.note ?? null },
    });
    if (!value) return false;
    return this.run(() =>
      firstValueFrom(
        this.api.updatePayeeGroup({ id: group.id, body: { name: value.name, note: value.note } }),
      ),
    );
  }

  /** Supprimer un groupe laisse ses tiers, sans groupe (R-TIE-4). */
  async removeGroup(group: PayeeGroupView): Promise<boolean> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer le groupe « ${group.name} »`,
      message: group.payee_count
        ? `Ses ${group.payee_count} tiers restent, sans groupe. Leurs transactions ne changent pas.`
        : 'Ce groupe ne contient aucun tiers.',
      action: 'Supprimer',
      destructive: true,
    });
    return ok ? this.run(() => firstValueFrom(this.api.deletePayeeGroup({ id: group.id }))) : false;
  }
}
