import { WIDGETS, defaultLayout, normalize } from './home-layout';

describe('disposition de l’accueil', () => {
  it('reprend la disposition par défaut sans sauvegarde', () => {
    expect(normalize(null)).toEqual(defaultLayout());
  });

  it('garde chaque encadré une fois, retire les inconnus et ajoute les nouveaux', () => {
    const state = normalize({
      desktop: [['accounts', 'inconnu', 'accounts'] as never, ['month']],
      mobile: ['budget'],
    });
    expect(state.desktop[0].slice(0, 1)).toEqual(['accounts']);
    expect(state.desktop[1]).toEqual(['month']);
    expect([...state.desktop.flat()].sort()).toEqual([...WIDGETS].sort());
    expect(state.mobile[0]).toBe('budget');
    expect(state.mobile.length).toBe(WIDGETS.length);
  });
});
