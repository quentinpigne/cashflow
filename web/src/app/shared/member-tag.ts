import { ChangeDetectionStrategy, Component, input } from '@angular/core';

/** Ce qu'affiche l'étiquette des membres d'une transaction (E3, E14). */
export interface MemberTagView {
  label: string;
  /** Couleur du membre ; absente quand plusieurs membres. */
  color: string | null;
}

/** Étiquette des membres d'une transaction, après son libellé : pastille de couleur et nom (R-MBR-5). */
@Component({
  selector: 'app-member-tag',
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `<span class="dot" [style.background]="tag().color ?? 'var(--mat-sys-outline)'"></span
    >{{ tag().label }}`,
  host: { '[attr.title]': "'Membre : ' + tag().label" },
  styles: `
    :host {
      display: inline-flex;
      align-items: center;
      gap: 0.3rem;
      margin-left: 0.5rem;
      padding: 0 0.5rem;
      border-radius: 1rem;
      background: var(--mat-sys-surface-container-high);
      color: var(--mat-sys-on-surface-variant);
      font: var(--mat-sys-label-small);
      line-height: 1.25rem;
      vertical-align: middle;
      white-space: nowrap;
    }
    .dot {
      width: 0.5rem;
      height: 0.5rem;
      border-radius: 50%;
      flex: none;
    }
  `,
})
export class MemberTag {
  readonly tag = input.required<MemberTagView>();
}
