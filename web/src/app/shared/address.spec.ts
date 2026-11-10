import { countryList } from './address';

describe('countryList', () => {
  it('propose chaque pays une seule fois, sous son code actuel (R-TIE-23)', () => {
    const list = countryList();
    const names = list.map((c) => c.name);
    expect(new Set(names).size).toBe(names.length);
    expect(list[0]).toEqual({ code: 'FR', name: 'France' });
    expect(list.filter((c) => c.name === 'Allemagne')).toEqual([{ code: 'DE', name: 'Allemagne' }]);
    expect(list.some((c) => ['DD', 'SU', 'YU', 'FX'].includes(c.code))).toBe(false);
  });
});
