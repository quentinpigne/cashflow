import { Injectable } from '@angular/core';
import { NativeDateAdapter } from '@angular/material/core';

/**
 * Adaptateur de dates français pour les sélecteurs Material : saisie `jj/mm/aaaa`, semaine commençant
 * le lundi (ecrans.md, principes).
 */
@Injectable()
export class FrDateAdapter extends NativeDateAdapter {
  override getFirstDayOfWeek(): number {
    return 1;
  }

  override parse(value: unknown, parseFormat?: unknown): Date | null {
    if (typeof value === 'string') {
      const match = /^\s*(\d{1,2})[/.-](\d{1,2})[/.-](\d{2}|\d{4})\s*$/.exec(value);
      if (match) {
        const day = Number(match[1]);
        const month = Number(match[2]) - 1;
        const year = match[3].length === 2 ? 2000 + Number(match[3]) : Number(match[3]);
        const date = new Date(year, month, day);
        return date.getMonth() === month && date.getDate() === day ? date : this.invalid();
      }
      if (value.trim() === '') {
        return null;
      }
    }
    return super.parse(value, parseFormat);
  }
}
