import { formatDate, formatDayHeader, fromIsoDate, toIsoDate } from './dates';

describe('dates', () => {
  it('convertit sans glissement de fuseau', () => {
    expect(toIsoDate(fromIsoDate('2026-03-12'))).toBe('2026-03-12');
    expect(toIsoDate(new Date(2026, 0, 31))).toBe('2026-01-31');
  });

  it('formate à la française', () => {
    expect(formatDate('2026-03-12')).toBe('12 mars 2026');
    expect(formatDayHeader('2026-03-12')).toBe('Jeudi 12 mars');
  });
});
