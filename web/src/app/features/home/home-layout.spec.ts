import { WIDGETS, columnCount, defaultColumns, normalize, parse } from './home-layout';

describe('disposition de l’accueil', () => {
  it('adapte le nombre de colonnes à la largeur, de une à quatre', () => {
    expect(columnCount(360)).toBe(1);
    expect(columnCount(720)).toBe(2);
    expect(columnCount(1104)).toBe(3);
    expect(columnCount(1600)).toBe(4);
    expect(columnCount(3000)).toBe(4);
  });

  it('a une disposition par défaut complète pour chaque nombre de colonnes', () => {
    for (const count of [1, 2, 3, 4]) {
      const columns = defaultColumns(count);
      expect(columns.length).toBe(count);
      expect([...columns.flat()].sort()).toEqual([...WIDGETS].sort());
    }
  });

  it('garde chaque encadré une fois, retire les inconnus et ajoute les nouveaux', () => {
    const columns = normalize([['accounts', 'inconnu', 'accounts'], ['month']], 2);
    expect(columns[0].slice(0, 1)).toEqual(['accounts']);
    expect(columns[1]).toEqual(['month']);
    expect([...columns.flat()].sort()).toEqual([...WIDGETS].sort());
  });

  it('replie les colonnes en trop sur la dernière', () => {
    const columns = normalize([['month'], ['budget'], ['accounts']], 2);
    expect(columns[1].slice(0, 2)).toEqual(['budget', 'accounts']);
  });

  it('reprend la disposition de la version précédente', () => {
    const state = parse(null, { desktop: [['budget'], ['month']], mobile: ['accounts'] });
    expect(state[2][1]).toEqual(['month']);
    expect(state[1][0][0]).toBe('accounts');
    expect(state[1][0].length).toBe(WIDGETS.length);
  });
});
