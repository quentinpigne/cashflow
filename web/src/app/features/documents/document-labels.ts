import { DocumentKind } from '../../api/models/document-kind';
import { LinkKind } from '../../api/models/link-kind';
import { LinkView } from '../../api/models/link-view';
import { formatMoney } from '../../core/money';

/** Types de document, dans l'ordre de la spec (R-DOC-5). */
export const DOCUMENT_KINDS: readonly DocumentKind[] = [
  'BANK_STATEMENT',
  'PAYSLIP',
  'CONTRACT',
  'RECEIPT',
  'INVOICE',
  'TAX',
  'CERTIFICATE',
  'LETTER',
  'OTHER',
];

export const KIND_LABELS: Record<DocumentKind, string> = {
  BANK_STATEMENT: 'Relevé de compte',
  PAYSLIP: 'Fiche de paie',
  CONTRACT: 'Contrat',
  RECEIPT: 'Ticket de caisse',
  INVOICE: 'Facture',
  TAX: 'Document fiscal',
  CERTIFICATE: 'Attestation',
  LETTER: 'Courrier',
  OTHER: 'Autre',
};

export const KIND_ICONS: Record<DocumentKind, string> = {
  BANK_STATEMENT: 'account_balance',
  PAYSLIP: 'work',
  CONTRACT: 'history_edu',
  RECEIPT: 'receipt',
  INVOICE: 'request_quote',
  TAX: 'gavel',
  CERTIFICATE: 'verified',
  LETTER: 'mail',
  OTHER: 'description',
};

/** Le relevé et la fiche de paie ont un mois et un objet propriétaire (R-DOC-7, R-DOC-10). */
export const OWNER_OF: Partial<Record<DocumentKind, LinkKind>> = {
  BANK_STATEMENT: 'ACCOUNT',
  PAYSLIP: 'EMPLOYMENT',
};

export function hasMonth(kind: DocumentKind | null | undefined): boolean {
  return !!kind && !!OWNER_OF[kind];
}

/** Natures d'objet auxquelles un document se rattache (R-DOC-9). */
export const LINK_KINDS: readonly LinkKind[] = [
  'ACCOUNT',
  'INSTITUTION',
  'EMPLOYMENT',
  'TRANSACTION',
  'ASSET',
  'LOAN',
  'PAYEE',
  'PROJECT',
];

export const LINK_LABELS: Record<LinkKind, string> = {
  ACCOUNT: 'Compte',
  INSTITUTION: 'Établissement',
  EMPLOYMENT: 'Emploi',
  TRANSACTION: 'Transaction',
  ASSET: 'Bien',
  LOAN: 'Prêt',
  PAYEE: 'Tiers',
  PROJECT: 'Projet',
};

export const LINK_ICONS: Record<LinkKind, string> = {
  ACCOUNT: 'account_balance_wallet',
  INSTITUTION: 'account_balance',
  EMPLOYMENT: 'work',
  TRANSACTION: 'receipt_long',
  ASSET: 'home_work',
  LOAN: 'real_estate_agent',
  PAYEE: 'storefront',
  PROJECT: 'flag',
};

/** Types de fichier acceptés (R-DOC-1) ; un relevé est un PDF. */
export const DOCUMENT_ACCEPT = '.pdf,.jpg,.jpeg,.png,application/pdf,image/jpeg,image/png';
export const PDF_ACCEPT = '.pdf,application/pdf';
export const DOCUMENT_MAX_BYTES = 10 * 1024 * 1024;

/** Adresse du fichier d'un document, affiché ou téléchargé (R-DOC-21). */
export function documentUrl(id: string, download = false): string {
  const url = `/api/v1/documents/${id}/content`;
  return download ? `${url}?download=true` : url;
}

/** Fiche de l'objet rattaché, s'il en a une. */
export function linkRoute(link: Pick<LinkView, 'kind' | 'id'>): string[] | null {
  switch (link.kind) {
    case 'ACCOUNT':
      return ['/comptes', link.id];
    case 'EMPLOYMENT':
      return ['/salaires', link.id];
    case 'TRANSACTION':
      return ['/transactions', link.id];
    case 'ASSET':
      return ['/biens', link.id];
    case 'LOAN':
      return ['/prets', link.id];
    case 'PAYEE':
      return ['/tiers', link.id];
    case 'PROJECT':
      return ['/budget/projets', link.id];
    case 'INSTITUTION':
      return ['/reglages/etablissements'];
  }
}

/** Libellé d'un rattachement : le nom de l'objet ; pour une transaction, avec sa date et son montant. */
export function linkLabel(link: LinkView): string {
  if (link.kind !== 'TRANSACTION' || !link.date || link.amount_minor == null) return link.name;
  const date = new Date(`${link.date}T00:00:00`).toLocaleDateString('fr-FR', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  });
  return `${link.name} · ${date} · ${formatMoney(link.amount_minor)}`;
}

/** « 245 Ko », « 1,2 Mo ». */
export function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} Ko`;
  return `${(bytes / (1024 * 1024)).toLocaleString('fr-FR', { maximumFractionDigits: 1 })} Mo`;
}

/** Extrait d'une recherche découpé en morceaux, mots trouvés marqués (entre U+0002 et U+0003). */
export function snippetParts(snippet: string): { text: string; hit: boolean }[] {
  const parts: { text: string; hit: boolean }[] = [];
  for (const piece of snippet.split('\u0002')) {
    const [hit, rest] = piece.includes('\u0003') ? piece.split('\u0003', 2) : [null, piece];
    if (hit !== null) parts.push({ text: hit, hit: true });
    if (rest) parts.push({ text: rest, hit: false });
  }
  return parts;
}
