import {
  formatDate,
  formatDayHeader,
  formatMediumDate,
  fromIsoDate,
  monthDiff,
  toIsoDate,
} from './dates';

describe('dates', () => {
  it('compte les mois entre deux mois, négatif vers le passé (R-IMP-29)', () => {
    expect(monthDiff('2026-10', '2026-11')).toBe(1);
    expect(monthDiff('2026-01', '2025-12')).toBe(-1);
    expect(monthDiff('2026-03', '2027-03')).toBe(12);
  });

  it('convertit sans glissement de fuseau', () => {
    expect(toIsoDate(fromIsoDate('2026-03-12'))).toBe('2026-03-12');
    expect(toIsoDate(new Date(2026, 0, 31))).toBe('2026-01-31');
  });

  it('formate à la française', () => {
    expect(formatDate('2026-03-12')).toBe('12 mars 2026');
    expect(formatMediumDate('2026-10-01')).toBe('1 oct. 2026');
    expect(formatDayHeader('2026-03-12')).toBe('Jeudi 12 mars');
  });
});
