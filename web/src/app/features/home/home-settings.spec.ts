import { normalizeSettings } from './home-settings';

describe('réglages des encadrés de l’accueil', () => {
  it('part de rien sans sauvegarde', () => {
    expect(normalizeSettings(null)).toEqual({ hidden: [], options: {} });
  });

  it('ne garde que les encadrés et les options connus', () => {
    const state = normalizeSettings({
      hidden: ['accounts', 'inconnu', 'accounts'],
      options: { 'month.previous': false, inconnue: true, 'accounts.detail': 'oui' },
    });
    expect(state).toEqual({ hidden: ['accounts'], options: { 'month.previous': false } });
  });
});
