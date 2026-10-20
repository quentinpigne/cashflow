import { Pipe, PipeTransform } from '@angular/core';

import { FormatOptions, formatMoney } from '../core/money';

/** `{{ amount_minor | money }}` → « 1 234,56 € ». */
@Pipe({ name: 'money' })
export class MoneyPipe implements PipeTransform {
  transform(
    minor: number | null | undefined,
    currency = 'EUR',
    options: Omit<FormatOptions, 'currency'> = {},
  ): string {
    return minor === null || minor === undefined
      ? '—'
      : formatMoney(minor, { ...options, currency });
  }
}
