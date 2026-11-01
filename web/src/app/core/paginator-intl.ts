import { Injectable } from '@angular/core';
import { MatPaginatorIntl } from '@angular/material/paginator';

/** Libellés du paginateur en français : « 51 – 100 sur 233 ». */
@Injectable()
export class FrPaginatorIntl extends MatPaginatorIntl {
  override itemsPerPageLabel = 'Par page';
  override nextPageLabel = 'Page suivante';
  override previousPageLabel = 'Page précédente';
  override firstPageLabel = 'Première page';
  override lastPageLabel = 'Dernière page';

  override getRangeLabel = (page: number, pageSize: number, length: number): string => {
    if (length === 0) return '0 sur 0';
    const start = page * pageSize + 1;
    const end = Math.min(start + pageSize - 1, length);
    return `${start} – ${end} sur ${length}`;
  };
}
