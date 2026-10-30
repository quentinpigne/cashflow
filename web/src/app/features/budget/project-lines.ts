import { ProjectLineView } from '../../api/models/project-line-view';
import { ProjectStatus } from '../../api/models/project-status';

/**
 * Poste proposé pour une ventilation d'un projet : celui qui porte sa catégorie, sinon sa catégorie parente,
 * sinon aucun (hors postes) (R-BUD-27). Même règle que le serveur, pour présélectionner le poste.
 */
export function proposedLine(
  lines: ProjectLineView[],
  categoryId: string | null | undefined,
  parentId: string | null | undefined,
): string | null {
  const find = (id: string | null | undefined) =>
    id ? (lines.find((l) => l.category_ids.includes(id))?.id ?? null) : null;
  return find(categoryId) ?? find(parentId);
}

/** Libellés des statuts d'un projet (R-BUD-20). */
export const PROJECT_STATUS_LABELS: Record<ProjectStatus, string> = {
  PLANNED: 'En projet',
  ACTIVE: 'En cours',
  CLOSED: 'Terminé',
};

/** Suffixe d'un projet dans une liste de choix : rien s'il est en cours, sinon son statut. */
export function projectSuffix(status: ProjectStatus): string {
  return status === 'ACTIVE' ? '' : ` (${PROJECT_STATUS_LABELS[status].toLowerCase()})`;
}
