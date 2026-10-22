/** Libellés français des valeurs d'énumération de l'API (glossaire). */

import { AccountFamily } from '../api/models/account-family';
import { AccountKind } from '../api/models/account-kind';
import { AssetKind } from '../api/models/asset-kind';
import { BonusKind } from '../api/models/bonus-kind';
import { LoanKind } from '../api/models/loan-kind';
import { SalaryChangeReason } from '../api/models/salary-change-reason';
import { BatchStatus } from '../api/models/batch-status';
import { ClearingStatus } from '../api/models/clearing-status';
import { ImportDecision } from '../api/models/import-decision';
import { TransactionOrigin } from '../api/models/transaction-origin';

export const FAMILY_LABELS: Record<AccountFamily, string> = {
  CASH: 'Liquidités',
  FIXED_SAVINGS: 'Épargne à taux fixe',
  VARIABLE_SAVINGS: 'Épargne variable',
};

export const FAMILY_ORDER: AccountFamily[] = ['CASH', 'FIXED_SAVINGS', 'VARIABLE_SAVINGS'];

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
};

/** Types de biens (R-BIE-1). */
export const ASSET_KIND_LABELS: Record<AssetKind, string> = {
  REAL_ESTATE: 'Bien immobilier',
  VEHICLE: 'Véhicule',
  OTHER: 'Autre bien',
};

/** Types de prêts (R-PRE-1). */
export const LOAN_KIND_LABELS: Record<LoanKind, string> = {
  MORTGAGE: 'Prêt immobilier',
  CONSUMER: 'Prêt à la consommation',
  OTHER: 'Autre prêt',
};

/** Motifs d'une rémunération (R-SAL-2). */
export const SALARY_REASON_LABELS: Record<SalaryChangeReason, string> = {
  HIRE: 'Embauche',
  RAISE: 'Augmentation',
  PROMOTION: 'Promotion',
  TAX_RATE: 'Taux de prélèvement',
  OTHER: 'Autre',
};

/** Natures de prime (R-SAL-5). */
export const BONUS_KIND_LABELS: Record<BonusKind, string> = {
  THIRTEENTH_MONTH: '13e mois',
  BONUS: 'Prime',
  PROFIT_SHARING: 'Intéressement ou participation',
  OTHER: 'Autre',
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

export const DECISION_LABELS: Record<ImportDecision, string> = {
  CREATE: 'Créer',
  MATCH: 'Pointer',
  SKIP: 'Ignorer',
};

export const BATCH_STATUS_LABELS: Record<BatchStatus, string> = {
  REVIEWING: 'En revue',
  COMMITTED: 'Validé',
  CANCELLED: 'Annulé',
  REVERTED: 'Annulé après validation',
};
