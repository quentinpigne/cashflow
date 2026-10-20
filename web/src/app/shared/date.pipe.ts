import { Pipe, PipeTransform } from '@angular/core';

import { formatDate, formatDayHeader, formatShortDate } from '../core/dates';

/** `{{ iso | frDate }}` → « 12 mars 2026 » ; `'short'` → « 12 mars » ; `'day'` → « Jeudi 12 mars ». */
@Pipe({ name: 'frDate' })
export class FrDatePipe implements PipeTransform {
  transform(iso: string | null | undefined, style: 'long' | 'short' | 'day' = 'long'): string {
    if (!iso) {
      return '';
    }
    return style === 'short'
      ? formatShortDate(iso)
      : style === 'day'
        ? formatDayHeader(iso)
        : formatDate(iso);
  }
}
