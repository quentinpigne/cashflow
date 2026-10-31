import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';

/** Pastille d'un membre du foyer : son initiale sur sa couleur (spec 14). */
@Component({
  selector: 'app-member-chip',
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `<span
    class="chip"
    [style.background]="color()"
    [attr.title]="title()"
    [attr.aria-label]="title()"
    role="img"
    >{{ initial() }}</span
  >`,
  styles: `
    :host {
      display: inline-flex;
      flex: none;
      margin-right: 0.25rem;
      vertical-align: middle;
    }
    .chip {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 1.5rem;
      height: 1.5rem;
      border-radius: 50%;
      color: #fff;
      font: var(--mat-sys-label-medium);
      font-weight: 600;
    }
  `,
})
export class MemberChip {
  /** Le membre ; `null` : plusieurs membres. */
  readonly member = input.required<{ name: string; color?: string | null } | null>();

  protected readonly color = computed(() => this.member()?.color ?? 'var(--mat-sys-outline)');
  protected readonly initial = computed(() => {
    const m = this.member();
    return m ? m.name.trim().charAt(0).toUpperCase() : '+';
  });
  protected readonly title = computed(() => this.member()?.name ?? 'Plusieurs membres');
}
