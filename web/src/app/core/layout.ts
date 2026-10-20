import { BreakpointObserver } from '@angular/cdk/layout';
import { Injectable, inject } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { map } from 'rxjs';

/** Bascule mobile / ordinateur à 600 px (ecrans.md, principes). */
@Injectable({ providedIn: 'root' })
export class Layout {
  private readonly breakpoints = inject(BreakpointObserver);
  readonly mobile = toSignal(
    this.breakpoints.observe('(max-width: 599.98px)').pipe(map((state) => state.matches)),
    { initialValue: window.matchMedia('(max-width: 599.98px)').matches },
  );
}
