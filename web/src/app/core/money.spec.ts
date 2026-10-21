import {
  allocate,
  formatMoney,
  formatRate,
  formatRatePpm,
  parseMoney,
  parsePercent,
  parseRatePpm,
  toEditable,
} from './money';

describe('money', () => {
  it('formate et analyse les taux annuels', () => {
    expect(formatRate(17_000)).toBe('1,70\u00a0%');
    expect(formatRate(30_000)).toBe('3,00\u00a0%');
    expect(parseRatePpm('1,7')).toBe(17_000);
    expect(parseRatePpm('2.40 %')).toBe(24_000);
    expect(parseRatePpm('0,125')).toBe(1_250);
    expect(parseRatePpm('abc')).toBeNull();
  });

  it('formate à la française', () => {
    expect(formatMoney(123456)).toBe('1 234,56 €');
    expect(formatMoney(-9500)).toBe('−95,00 €');
    expect(formatMoney(5)).toBe('0,05 €');
    expect(formatMoney(1500, { currency: 'JPY', bare: true })).toBe('1\u202f500');
    expect(formatMoney(2000, { signed: true, bare: true })).toBe('+20,00');
  });

  it('analyse les saisies sans flottant', () => {
    expect(parseMoney('1 234,56')).toBe(123456);
    expect(parseMoney('1 234,5')).toBe(123450);
    expect(parseMoney('12.3')).toBe(1230);
    expect(parseMoney('-3')).toBe(-300);
    expect(parseMoney('95,00 €')).toBe(9500);
    expect(parseMoney('0,1')).toBe(10);
    expect(parseMoney('1,234')).toBeNull();
    expect(parseMoney('abc')).toBeNull();
    expect(parseMoney('')).toBeNull();
    // 0,1 + 0,2 : aucun flottant en jeu.
    expect((parseMoney('0,1') ?? 0) + (parseMoney('0,2') ?? 0)).toBe(30);
  });

  it('produit un texte éditable', () => {
    expect(toEditable(123456)).toBe('1234,56');
    expect(toEditable(-5)).toBe('-0,05');
  });

  it('analyse les pourcentages en centièmes de point', () => {
    expect(parsePercent('33,33')).toBe(3333);
    expect(parsePercent('50')).toBe(5000);
    expect(parsePercent('12,345')).toBeNull();
  });

  it('répartit comme le domaine (TRX-03, R-TRX-14)', () => {
    expect(allocate(-12000, [50, 30, 20])).toEqual([-6000, -3600, -2400]);
    expect(allocate(-10000, [1, 1, 1])).toEqual([-3333, -3333, -3334]);
    expect(allocate(-100, [])).toBeNull();
    expect(allocate(-100, [0, 0])).toBeNull();
  });

  it('formate un taux', () => {
    expect(formatRatePpm(416667)).toBe('41,7 %');
    expect(formatRatePpm(-50000)).toBe('−5,0 %');
  });
});
