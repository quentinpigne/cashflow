import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';

import { DocumentsService } from '../../api/services/documents.service';
import { DataVersion } from '../../core/data-version';
import { FrDatePipe } from '../../shared/date.pipe';
import { WidgetMenu } from '../home/widget-menu';

/** Documents sans type de la bibliothèque : leur nombre et les plus récents (TDB-12, R-TDB-24). */
@Component({
  selector: 'app-home-documents',
  imports: [RouterLink, WidgetMenu, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (unsorted().length > 0) {
      <section class="card">
        <h2 class="widget-title">
          <a routerLink="/documents" [queryParams]="{ filtre: 'a-classer' }"
            >Documents à classer ({{ unsorted().length }})</a
          >
        </h2>
        <app-widget-menu widget="documents" />
        @for (d of latest(); track d.id) {
          <a class="row" [routerLink]="['/documents', d.id]">
            <span class="label"
              >{{ d.title }}
              <span class="muted small">envoyé le {{ d.sent_on | frDate: 'short' }}</span></span
            >
          </a>
        }
        @if (unsorted().length > latest().length) {
          <a class="more" routerLink="/documents" [queryParams]="{ filtre: 'a-classer' }"
            >Voir tout</a
          >
        }
      </section>
    }
  `,
  styleUrl: '../home/widget.scss',
})
export class HomeDocuments {
  private readonly api = inject(DocumentsService);
  private readonly version = inject(DataVersion);

  private readonly documents = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listDocuments({ unsorted: true }),
  });

  protected readonly unsorted = computed(() => this.documents.value() ?? []);
  protected readonly latest = computed(() =>
    [...this.unsorted()].sort((a, b) => b.sent_on.localeCompare(a.sent_on)).slice(0, 3),
  );
}
