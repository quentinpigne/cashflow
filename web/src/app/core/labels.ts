/** Libellés français des valeurs d'énumération de l'API (glossaire). */

import { AccountFamily } from '../api/models/account-family';
import { AccountKind } from '../api/models/account-kind';
import { ClearingStatus } from '../api/models/clearing-status';
import { TransactionOrigin } from '../api/models/transaction-origin';

export const FAMILY_LABELS: Record<AccountFamily, string> = {
  CASH: 'Liquidités',
  FIXED_SAVINGS: 'Épargne à taux fixe',
  VARIABLE_SAVINGS: 'Épargne variable',
  LIABILITY: 'Passifs',
};

export const FAMILY_ORDER: AccountFamily[] = [
  'CASH',
  'FIXED_SAVINGS',
  'VARIABLE_SAVINGS',
  'LIABILITY',
];

export const KIND_LABELS: Record<AccountKind, string> = {
  CHECKING: 'Compte courant',
  CASH_WALLET: 'Espèces',
  OTHER_CASH: 'Autre liquidité',
  LIVRET_A: 'Livret A',
  LDDS: 'LDDS',
  LEP: 'LEP',
  LIVRET_JEUNE: 'Livret Jeune',
  CEL: 'CEL',
  PEL: 'PEL',
  TERM_DEPOSIT: 'Compte à terme',
  SAVINGS_OTHER: 'Autre épargne à taux fixe',
  LIFE_INSURANCE: 'Assurance vie',
  PEA: 'PEA',
  PEA_PME: 'PEA-PME',
  PER: 'PER',
  SECURITIES: 'Compte-titres',
  EMPLOYEE_SAVINGS: 'Épargne salariale',
  CRYPTO: 'Crypto-actifs',
  VARIABLE_OTHER: 'Autre épargne variable',
  MORTGAGE: 'Prêt immobilier',
  CONSUMER_LOAN: 'Prêt à la consommation',
  LOAN_OTHER: 'Autre prêt',
};

/** Famille de chaque nature (spec 01), pour grouper la liste des natures. */
export const KIND_FAMILY: Record<AccountKind, AccountFamily> = {
  CHECKING: 'CASH',
  CASH_WALLET: 'CASH',
  OTHER_CASH: 'CASH',
  LIVRET_A: 'FIXED_SAVINGS',
  LDDS: 'FIXED_SAVINGS',
  LEP: 'FIXED_SAVINGS',
  LIVRET_JEUNE: 'FIXED_SAVINGS',
  CEL: 'FIXED_SAVINGS',
  PEL: 'FIXED_SAVINGS',
  TERM_DEPOSIT: 'FIXED_SAVINGS',
  SAVINGS_OTHER: 'FIXED_SAVINGS',
  LIFE_INSURANCE: 'VARIABLE_SAVINGS',
  PEA: 'VARIABLE_SAVINGS',
  PEA_PME: 'VARIABLE_SAVINGS',
  PER: 'VARIABLE_SAVINGS',
  SECURITIES: 'VARIABLE_SAVINGS',
  EMPLOYEE_SAVINGS: 'VARIABLE_SAVINGS',
  CRYPTO: 'VARIABLE_SAVINGS',
  VARIABLE_OTHER: 'VARIABLE_SAVINGS',
  MORTGAGE: 'LIABILITY',
  CONSUMER_LOAN: 'LIABILITY',
  LOAN_OTHER: 'LIABILITY',
};

export const STATUS_LABELS: Record<ClearingStatus, string> = {
  PENDING: 'En attente',
  CLEARED: 'Pointée',
  RECONCILED: 'Rapprochée',
};

export const ORIGIN_LABELS: Record<TransactionOrigin, string> = {
  MANUAL: 'Saisie manuelle',
  IMPORT: 'Import',
  RECURRING: 'Récurrence',
  INTEREST: 'Intérêts',
};

/** Natures dont le solde se calcule à partir des transactions (R-CPT-4) ; sinon c'est une valeur (R-CPT-6). */
export function hasComputedBalance(family: AccountFamily): boolean {
  return family === 'CASH' || family === 'FIXED_SAVINGS';
}

/** Valeurs par défaut des options d'un compte selon sa nature (R-CPT-11, R-CPT-13). */
export function defaultFlags(kind: AccountKind): { emergency: boolean; budget: boolean } {
  const family = KIND_FAMILY[kind];
  return {
    emergency: family === 'CASH' || ['LIVRET_A', 'LDDS', 'LEP', 'LIVRET_JEUNE'].includes(kind),
    budget: family === 'CASH',
  };
}
