import { ProjectLineView } from '../../api/models/project-line-view';

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
