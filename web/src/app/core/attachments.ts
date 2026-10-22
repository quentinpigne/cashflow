/**
 * Rattachement d'une ventilation, codé dans le choix de catégorie : un identifiant de catégorie, `loan:<id>` pour
 * un remboursement de prêt (R-PRE-7) ou `asset:<id>` pour une acquisition de bien (R-BIE-8) (R-TRX-37).
 */
export interface SplitAttachment {
  category_id: string | null;
  loan_id: string | null;
  asset_id: string | null;
}

export function decodeAttachment(value: string | null | undefined): SplitAttachment {
  if (value?.startsWith('loan:'))
    return { category_id: null, loan_id: value.slice(5), asset_id: null };
  if (value?.startsWith('asset:'))
    return { category_id: null, loan_id: null, asset_id: value.slice(6) };
  return { category_id: value ?? null, loan_id: null, asset_id: null };
}

export function encodeAttachment(split: {
  category_id?: string | null;
  loan_id?: string | null;
  asset_id?: string | null;
}): string | null {
  if (split.loan_id) return `loan:${split.loan_id}`;
  if (split.asset_id) return `asset:${split.asset_id}`;
  return split.category_id ?? null;
}

/** La valeur désigne-t-elle un prêt ou un bien plutôt qu'une catégorie ? */
export function isAttachment(value: string | null | undefined): boolean {
  return !!value && (value.startsWith('loan:') || value.startsWith('asset:'));
}
