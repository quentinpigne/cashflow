import { Account } from '../../api/models/account';
import { AccountFamily } from '../../api/models/account-family';
import { FAMILY_LABELS, FAMILY_ORDER } from '../../core/labels';
import { sum } from '../../core/money';

export interface AccountGroup {
  family: AccountFamily;
  label: string;
  /** Total des comptes en euros (R-CPT-2). */
  totalMinor: number;
  accounts: Account[];
}

/** Comptes groupés par famille, dans l'ordre de la spec 01, avec le total de chaque groupe. */
export function groupByFamily(accounts: Account[]): AccountGroup[] {
  return FAMILY_ORDER.map((family) => {
    const members = accounts.filter((a) => a.family === family);
    return {
      family,
      label: FAMILY_LABELS[family],
      totalMinor: sum(members.filter((a) => a.currency === 'EUR').map((a) => a.balance_minor)),
      accounts: members,
    };
  }).filter((g) => g.accounts.length > 0);
}
