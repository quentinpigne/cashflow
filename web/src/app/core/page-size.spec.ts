import { loadPageSize, savePageSize } from './page-size';

/** Stockage en mémoire : l'environnement de test n'a pas de `localStorage`. */
function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => values.delete(key),
    setItem: (key, value) => values.set(key, value),
  } as Storage;
}

describe('taille de page', () => {
  beforeEach(() => {
    Object.defineProperty(globalThis, 'localStorage', {
      value: memoryStorage(),
      configurable: true,
    });
  });

  it('part de la valeur par défaut, puis retient le choix', () => {
    expect(loadPageSize('test', 50)).toBe(50);
    savePageSize('test', 200);
    expect(loadPageSize('test', 50)).toBe(200);
  });

  it('ignore une taille qui n’est pas proposée', () => {
    localStorage.setItem('cashflow.page-size.test', '7');
    expect(loadPageSize('test', 100)).toBe(100);
  });
});
