import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  computed,
  inject,
  signal,
} from '@angular/core';
import { takeUntilDestroyed, toObservable, toSignal } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatChipsModule } from '@angular/material/chips';
import { MatDialog } from '@angular/material/dialog';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router } from '@angular/router';
import {
  Observable,
  catchError,
  debounceTime,
  distinctUntilChanged,
  filter,
  firstValueFrom,
  map,
  of,
  skip,
  switchMap,
} from 'rxjs';

import { ComputeInput } from '../../api/models/compute-input';
import { ComputeResult } from '../../api/models/compute-result';
import { CreditConditions } from '../../api/models/credit-conditions';
import { FinancingRequest } from '../../api/models/financing-request';
import { FinancingView } from '../../api/models/financing-view';
import { LoanCostView } from '../../api/models/loan-cost-view';
import { ProjectDefaults } from '../../api/models/project-defaults';
import { ProjectKind } from '../../api/models/project-kind';
import { PurchaseCapacity } from '../../api/models/purchase-capacity';
import { SimulateQuestion } from '../../api/models/simulate-question';
import { SimulateState } from '../../api/models/simulate-state';
import { SimulationGoal } from '../../api/models/simulation-goal';
import { SimulationView } from '../../api/models/simulation-view';
import { SituationView } from '../../api/models/situation-view';
import { CreditService } from '../../api/services/credit.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { Layout } from '../../core/layout';
import { formatMoney, formatRate, parseMoney } from '../../core/money';
import { MoneyPipe } from '../../shared/money.pipe';
import { editable } from './applications';
import { ConditionsFields, PROJECT_SHORT_LABELS, conditionsSummary } from './conditions';
import { LoansTabs, editSimulation, formatDuration, showSchedule } from './credit-shared';
import { Affordability, CapacityDetails, LoanFigures, tenthsOfMonths } from './results';
import { SimulationsPage } from './simulations';
import { SituationBar } from './situation';

const PROJECTS: ProjectKind[] = ['EXISTING_HOME', 'NEW_HOME', 'WORKS', 'VEHICLE', 'OTHER'];

/** Le résultat de la question posée, ou ce qui manque pour le calculer. */
type Outcome =
  | { question: 'MAX_PRICE'; capacity: PurchaseCapacity }
  | { question: 'FINANCE_PURCHASE'; financing: FinancingView }
  | { question: 'LOAN'; cost: LoanCostView }
  | { question: null; error: string };

/** Ce qu'un calcul enregistre en simulation : le capital et la durée trouvés, aux conditions du calcul. */
interface Values {
  principal_minor: number;
  duration_months: number;
  name: string;
}

/** E36 — Simuler : ma situation, un projet, une question à la fois, le résultat en vue (CRE-07, CRE-08, CRE-11). */
@Component({
  selector: 'app-simulate',
  imports: [
    FormsModule,
    MatButtonModule,
    MatButtonToggleModule,
    MatCheckboxModule,
    MatChipsModule,
    MatExpansionModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MoneyPipe,
    LoansTabs,
    SituationBar,
    ConditionsFields,
    Affordability,
    CapacityDetails,
    LoanFigures,
    SimulationsPage,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './simulate.html',
  styleUrl: './simulate.scss',
})
export class SimulatePage {
  private readonly api = inject(CreditService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);

  protected readonly mobile = inject(Layout).mobile;
  protected readonly projects = PROJECTS;
  protected readonly projectLabels = PROJECT_SHORT_LABELS;
  protected readonly rate = formatRate;
  protected readonly durationText = formatDuration;
  protected readonly months = tenthsOfMonths;
  protected readonly summary = conditionsSummary;

  protected readonly situation = signal<SituationView | null>(null);
  private readonly defaults = signal<ProjectDefaults[]>([]);
  private readonly loaded = signal(false);

  // Le dernier état de l'onglet, gardé d'une visite à l'autre (R-CRE-39).
  protected readonly project = signal<ProjectKind>('EXISTING_HOME');
  protected readonly question = signal<SimulateQuestion>('MAX_PRICE');
  protected readonly conditions = signal<CreditConditions | null>(null);
  protected readonly price = signal('');
  /** Vide : l'apport proposé. */
  protected readonly downPayment = signal('');
  /** Vides : les frais de notaire estimés. */
  protected readonly notaryFees = signal('');
  protected readonly fixPayment = signal(false);
  protected readonly goal = signal<SimulationGoal>('PAYMENT');
  protected readonly principal = signal('');
  protected readonly target = signal('');
  /** Simulation enregistrée ouverte dans « Que coûte ce prêt ? » (R-CRE-37). */
  protected readonly editing = signal<SimulationView | null>(null);

  protected readonly isHome = computed(
    () => this.project() === 'EXISTING_HOME' || this.project() === 'NEW_HOME',
  );
  /** La durée est cherchée : son champ disparaît des conditions. */
  protected readonly durationComputed = computed(
    () =>
      (this.question() === 'FINANCE_PURCHASE' && this.fixPayment()) ||
      (this.question() === 'LOAN' && this.goal() === 'DURATION'),
  );

  /** La requête de la question posée, ou ce qui manque. */
  private readonly request = computed<
    { q: SimulateQuestion; body: unknown; situation: unknown } | string
  >(() => {
    if (!this.loaded()) return 'Chargement…';
    const conditions = this.conditions();
    if (!conditions) return 'Conditions incomplètes.';
    const project_kind = this.project();
    // La situation fait partie de la requête : la changer relance le calcul.
    const situation = this.situation()?.situation;
    switch (this.question()) {
      case 'MAX_PRICE':
        return { q: 'MAX_PRICE', body: { project_kind, conditions }, situation };
      case 'FINANCE_PURCHASE': {
        const body = this.financingRequest(project_kind, conditions);
        return typeof body === 'string' ? body : { q: 'FINANCE_PURCHASE', body, situation };
      }
      case 'LOAN': {
        const loan = this.loanInput(conditions);
        return typeof loan === 'string'
          ? loan
          : { q: 'LOAN', body: { project_kind, loan }, situation };
      }
    }
  });

  private financingRequest(
    project_kind: ProjectKind,
    conditions: CreditConditions,
  ): FinancingRequest | string {
    const price = parseMoney(this.price());
    if (!price || price <= 0) return "Saisissez le prix de l'achat.";
    const down = this.downPayment().trim() ? parseMoney(this.downPayment()) : null;
    if (this.downPayment().trim() && (down === null || down < 0)) return 'Apport invalide.';
    const notary = this.notaryFees().trim() ? parseMoney(this.notaryFees()) : null;
    if (this.notaryFees().trim() && (notary === null || notary < 0))
      return 'Frais de notaire invalides.';
    const target = this.fixPayment() ? parseMoney(this.target()) : null;
    if (this.fixPayment() && (!target || target <= 0)) return 'Saisissez la mensualité visée.';
    return {
      project_kind,
      conditions,
      price_minor: price,
      down_payment_minor: down,
      notary_fees_minor: this.isHome() ? notary : null,
      target_payment_minor: target,
    };
  }

  private loanInput(c: CreditConditions): ComputeInput | string {
    const goal = this.goal();
    const principal = parseMoney(this.principal());
    const target = parseMoney(this.target());
    if (goal !== 'PRINCIPAL' && (!principal || principal <= 0)) return 'Saisissez le capital.';
    if (goal !== 'PAYMENT' && (!target || target <= 0)) return 'Saisissez la mensualité visée.';
    return {
      goal,
      kind: c.kind,
      principal_minor: goal === 'PRINCIPAL' ? null : principal,
      duration_months: goal === 'DURATION' ? null : c.duration_months,
      target_payment_minor: goal === 'PAYMENT' ? null : target,
      annual_rate_ppm: c.annual_rate_ppm,
      insurance: c.insurance,
      fees_minor: c.fees_minor,
      guarantee_fees_minor: c.guarantee_fees_minor,
    };
  }

  protected readonly busy = signal(false);

  /** Le résultat, recalculé à chaque saisie. */
  protected readonly outcome = toSignal(
    toObservable(this.request).pipe(
      debounceTime(250),
      distinctUntilChanged((a, b) => JSON.stringify(a) === JSON.stringify(b)),
      switchMap((r): Observable<Outcome> => {
        if (typeof r === 'string') return of({ question: null, error: r });
        this.busy.set(true);
        const call: Observable<Outcome> =
          r.q === 'MAX_PRICE'
            ? this.api
                .purchaseCapacity({ body: r.body as never })
                .pipe(map((capacity) => ({ question: 'MAX_PRICE' as const, capacity })))
            : r.q === 'FINANCE_PURCHASE'
              ? this.api
                  .purchaseFinancing({ body: r.body as FinancingRequest })
                  .pipe(map((financing) => ({ question: 'FINANCE_PURCHASE' as const, financing })))
              : this.api
                  .loanCost({ body: r.body as never })
                  .pipe(map((cost) => ({ question: 'LOAN' as const, cost })));
        return call.pipe(catchError((e) => of({ question: null, error: errorMessage(e) })));
      }),
      map((o) => {
        this.busy.set(false);
        return o;
      }),
    ),
  );

  protected readonly capacity = computed(() => {
    const o = this.outcome();
    return o?.question === 'MAX_PRICE' ? o.capacity : null;
  });
  protected readonly financing = computed(() => {
    const o = this.outcome();
    return o?.question === 'FINANCE_PURCHASE' ? o.financing : null;
  });
  protected readonly cost = computed(() => {
    const o = this.outcome();
    return o?.question === 'LOAN' ? o.cost : null;
  });
  protected readonly error = computed(() => {
    const o = this.outcome();
    return o?.question === null ? o.error : null;
  });
  /** Le crédit calculé, pour le tableau d'amortissement. */
  protected readonly loan = computed<ComputeResult | null>(
    () => this.financing()?.loan ?? this.cost()?.loan ?? null,
  );

  /** Le chiffre clé, rappelé en bas d'écran sur mobile. */
  protected readonly headline = computed(() => {
    const c = this.capacity();
    if (c) return `Prix maximal ${formatMoney(c.today.max_price_minor)}`;
    const f = this.financing();
    if (f) {
      if (!f.loan) return 'Achat comptant, sans crédit';
      return `${formatMoney(f.loan.result.total_payment_minor)}/mois · ${this.ratio(f.affordability.debt_ratio_ppm)}`;
    }
    const l = this.cost();
    if (l)
      return `${formatMoney(l.loan.result.total_payment_minor)}/mois · ${this.ratio(l.affordability.debt_ratio_ppm)}`;
    return null;
  });

  private ratio(ppm: number | null | undefined): string {
    return ppm === null || ppm === undefined ? '—' : `endettement ${formatRate(ppm)}`;
  }

  /** L'état gardé (R-CRE-39). */
  private readonly state = computed<SimulateState | null>(() => {
    const conditions = this.conditions();
    if (!this.loaded() || !conditions) return null;
    const amount = (text: string) => (text.trim() ? parseMoney(text) : null);
    return {
      project_kind: this.project(),
      question: this.question(),
      conditions,
      price_minor: amount(this.price()),
      down_payment_minor: amount(this.downPayment()),
      notary_fees_minor: amount(this.notaryFees()),
      goal: this.goal(),
      principal_minor: amount(this.principal()),
      target_payment_minor: amount(this.target()),
      fix_payment: this.fixPayment(),
    };
  });

  constructor() {
    void this.load();
    toObservable(this.state)
      .pipe(
        filter((s): s is SimulateState => s !== null),
        distinctUntilChanged((a, b) => JSON.stringify(a) === JSON.stringify(b)),
        skip(1),
        debounceTime(600),
        switchMap((body) => this.api.saveSimulateState({ body }).pipe(catchError(() => of(null)))),
        takeUntilDestroyed(inject(DestroyRef)),
      )
      .subscribe();
  }

  private async load(): Promise<void> {
    try {
      const [state, situation, defaults] = await Promise.all([
        firstValueFrom(this.api.simulateState()),
        firstValueFrom(this.api.borrowerSituation()),
        firstValueFrom(this.api.projectDefaults()),
      ]);
      const text = (minor: number | null | undefined) => (minor ? editable(minor) : '');
      this.project.set(state.project_kind);
      this.question.set(state.question);
      this.conditions.set(state.conditions);
      this.price.set(text(state.price_minor));
      this.downPayment.set(text(state.down_payment_minor));
      this.notaryFees.set(text(state.notary_fees_minor));
      this.goal.set(state.goal);
      this.principal.set(text(state.principal_minor));
      this.target.set(text(state.target_payment_minor));
      this.fixPayment.set(state.fix_payment);
      this.situation.set(situation);
      this.defaults.set(defaults);
      this.loaded.set(true);
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }

  private defaultsOf(project: ProjectKind): CreditConditions | null {
    return this.defaults().find((d) => d.project_kind === project)?.conditions ?? null;
  }

  /** Choisir un projet charge ses conditions par défaut (R-CRE-39). */
  protected selectProject(project: ProjectKind): void {
    if (!project || project === this.project()) return;
    this.project.set(project);
    this.notaryFees.set('');
    this.resetConditions();
  }

  protected resetConditions(): void {
    const defaults = this.defaultsOf(this.project());
    if (defaults) this.conditions.set(defaults);
    this.editing.set(null);
  }

  protected readonly conditionsAreDefaults = computed(() => {
    const d = this.defaultsOf(this.project());
    return !!d && JSON.stringify(d) === JSON.stringify(this.conditions());
  });

  protected selectQuestion(question: SimulateQuestion): void {
    this.question.set(question);
    if (question !== 'LOAN') this.editing.set(null);
  }

  /** Une simulation ouverte passe dans « Que coûte ce prêt ? », sans toucher au projet ni à la situation. */
  protected open(s: SimulationView): void {
    this.editing.set(s);
    this.question.set('LOAN');
    this.goal.set('PAYMENT');
    this.principal.set(editable(s.principal_minor));
    this.conditions.set({
      kind: s.kind,
      annual_rate_ppm: s.annual_rate_ppm,
      duration_months: s.duration_months,
      insurance: s.insurance,
      fees_minor: s.fees_minor,
      guarantee_fees_minor: s.guarantee_fees_minor,
    });
  }

  /** Repart du capital empruntable : « Que coûte ce prêt ? » préremplie. */
  protected startFromCapacity(capital: number): void {
    this.editing.set(null);
    this.goal.set('PAYMENT');
    this.principal.set(editable(capital));
    this.question.set('LOAN');
  }

  /** Ce que la question enregistre en simulation. */
  private values(): Values | null {
    const c = this.conditions();
    if (!c) return null;
    const capacity = this.capacity();
    if (capacity) {
      if (capacity.today.capital_minor <= 0) return null;
      return {
        principal_minor: capacity.today.capital_minor,
        duration_months: c.duration_months,
        name: `Achat jusqu'à ${formatMoney(capacity.today.max_price_minor)}`,
      };
    }
    const f = this.financing();
    if (f?.loan)
      return {
        principal_minor: f.loan.principal_minor,
        duration_months: f.loan.duration_months,
        name: `${PROJECT_SHORT_LABELS[this.project()]} à ${formatMoney(f.price_minor)}`,
      };
    const l = this.cost()?.loan;
    if (l)
      return {
        principal_minor: l.principal_minor,
        duration_months: l.duration_months,
        name: `${formatDuration(l.duration_months)} à ${formatRate(c.annual_rate_ppm)}`,
      };
    return null;
  }

  private body(v: Values, c: CreditConditions) {
    return {
      kind: c.kind,
      principal_minor: v.principal_minor,
      duration_months: v.duration_months,
      annual_rate_ppm: c.annual_rate_ppm,
      insurance: c.insurance,
      fees_minor: c.fees_minor,
      guarantee_fees_minor: c.guarantee_fees_minor,
    };
  }

  protected readonly canSave = computed(() => {
    this.outcome();
    return this.values() !== null;
  });

  /** Enregistre le calcul en simulation (R-CRE-37). */
  protected async save(): Promise<void> {
    const v = this.values();
    const c = this.conditions();
    if (!v || !c) return;
    const initial = { ...this.body(v, c), name: v.name };
    if (await editSimulation(this.dialog, { simulation: null, initial })) this.version.bump();
  }

  /** Met à jour la simulation ouverte. */
  protected async update(s: SimulationView): Promise<void> {
    const v = this.values();
    const c = this.conditions();
    if (!v || !c) return;
    try {
      const saved = await firstValueFrom(
        this.api.updateLoanSimulation({
          id: s.id,
          body: {
            ...this.body(v, c),
            name: s.name,
            institution_id: s.institution_id ?? null,
            application_id: s.application_id ?? null,
            note: s.note ?? null,
          },
        }),
      );
      this.editing.set(saved);
      this.version.bump();
      this.snack.open('Simulation enregistrée', undefined, { duration: 3000 });
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }

  /** Prépare une demande depuis la question posée (R-CRE-36). */
  protected async prepare(): Promise<void> {
    const v = this.values();
    const c = this.conditions();
    const project_kind = this.project();
    if (!v || !c) return;
    try {
      let id: string;
      const request = this.request();
      if (this.question() === 'MAX_PRICE') {
        const body = { name: v.name, project_kind, conditions: c };
        id = (await firstValueFrom(this.api.applicationFromCapacity({ body }))).id;
      } else if (this.question() === 'FINANCE_PURCHASE' && typeof request !== 'string') {
        const body = { ...(request.body as FinancingRequest), name: v.name };
        id = (await firstValueFrom(this.api.applicationFromFinancing({ body }))).id;
      } else {
        const sim = await firstValueFrom(
          this.api.createLoanSimulation({
            body: { ...this.body(v, c), name: v.name, institution_id: null, application_id: null },
          }),
        );
        const body = { name: v.name, project_kind };
        id = (await firstValueFrom(this.api.applicationFromSimulation({ id: sim.id, body }))).id;
      }
      this.version.bump();
      await this.router.navigate(['/prets/demandes', id]);
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }

  protected schedule(): void {
    const v = this.values();
    const loan = this.loan();
    if (v && loan) showSchedule(this.dialog, { name: v.name, installments: loan.installments });
  }
}
