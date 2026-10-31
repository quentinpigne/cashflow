import { computed, signal } from '@angular/core';

import { MemberView } from '../api/models/member-view';
import { HOUSEHOLD, MemberSelection } from './member-filter';
import { MembersStore } from './stores';

const member = (id: string, name: string): MemberView => ({
  id,
  name,
  color: null,
  display_order: 0,
  account_count: 0,
  split_count: 0,
  employment_count: 0,
  asset_count: 0,
  document_count: 0,
});

function make(relevant: string[], screen: `home.${string}` = `home.test-${Math.random()}`) {
  const list = signal([member('q', 'Quentin'), member('c', 'Claire')]);
  const store = { active: computed(() => true), list } as unknown as MembersStore;
  const relevance = signal(new Set(relevant));
  return { selection: new MemberSelection(screen, store, relevance), relevance };
}

describe('MemberSelection (R-MBR-8)', () => {
  it('ne propose que les cases qui comptent quelque chose', () => {
    const { selection } = make(['q', 'c']);
    expect(selection.options().map((o) => o.name)).toEqual(['Quentin', 'Claire']);
    expect(selection.menuItems()[0].section).toBe('Membres');
  });

  it("ne propose rien quand il ne reste qu'une case", () => {
    const { selection } = make([HOUSEHOLD]);
    expect(selection.options()).toEqual([]);
    expect(selection.menuItems()).toEqual([]);
    expect(selection.param()).toBeUndefined();
  });

  it('revient à tout cocher quand la case cochée n’est plus proposée', () => {
    const { selection, relevance } = make([HOUSEHOLD, 'q', 'c']);
    selection.toggle(HOUSEHOLD, false);
    selection.toggle('q', false);
    expect(selection.param()).toBe('c');
    expect(selection.titleSuffix()).toBe(' · Claire');
    relevance.set(new Set([HOUSEHOLD, 'q']));
    expect(selection.checked()).toEqual([HOUSEHOLD, 'q']);
    expect(selection.partial()).toBe(false);
  });
});
