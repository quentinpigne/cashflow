import { Pipe, PipeTransform } from '@angular/core';

import {
  formatDate,
  formatDayHeader,
  formatMediumDate,
  formatMonth,
  formatShortDate,
} from '../core/dates';

/**
 * `{{ iso | frDate }}` → « 12 mars 2026 » ; `'medium'` → « 1 oct. 2026 » ; `'short'` → « 12 mars » ;
 * `'day'` → « Jeudi 12 mars » ; `'month'` → « Mars 2026 ».
 */
@Pipe({ name: 'frDate' })
export class FrDatePipe implements PipeTransform {
  transform(
    iso: string | null | undefined,
    style: 'long' | 'medium' | 'short' | 'day' | 'month' = 'long',
  ): string {
    if (!iso) {
      return '';
    }
    switch (style) {
      case 'medium':
        return formatMediumDate(iso);
      case 'short':
        return formatShortDate(iso);
      case 'day':
        return formatDayHeader(iso);
      case 'month':
        return formatMonth(iso);
      default:
        return formatDate(iso);
    }
  }
}
