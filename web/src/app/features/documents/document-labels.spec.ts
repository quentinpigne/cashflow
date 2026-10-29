import { describe, expect, it } from 'vitest';

import { formatSize, snippetParts } from './document-labels';

describe('snippetParts (R-DOC-18)', () => {
  it('marque les mots trouvés entre U+0002 et U+0003', () => {
    expect(snippetParts('…la \u0002résiliation\u0003 du \u0002contrat\u0003.')).toEqual([
      { text: '…la ', hit: false },
      { text: 'résiliation', hit: true },
      { text: ' du ', hit: false },
      { text: 'contrat', hit: true },
      { text: '.', hit: false },
    ]);
  });

  it('laisse un texte sans marque tel quel', () => {
    expect(snippetParts('aucun mot')).toEqual([{ text: 'aucun mot', hit: false }]);
  });
});

describe('formatSize', () => {
  it('donne des Ko puis des Mo', () => {
    expect(formatSize(250_000)).toBe('244 Ko');
    expect(formatSize(1_300_000)).toBe('1,2 Mo');
  });
});
