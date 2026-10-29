import { Routes } from '@angular/router';

import { requireSession } from './core/auth';

export const routes: Routes = [
  {
    path: 'connexion',
    title: 'Connexion — Cashflow',
    loadComponent: () => import('./features/login/login').then((m) => m.LoginPage),
  },
  {
    path: '',
    canActivate: [requireSession],
    loadComponent: () => import('./features/shell/shell').then((m) => m.Shell),
    children: [
      {
        path: '',
        title: 'Accueil — Cashflow',
        loadComponent: () => import('./features/home/home').then((m) => m.HomePage),
      },
      {
        path: 'comptes',
        title: 'Comptes — Cashflow',
        loadComponent: () => import('./features/accounts/accounts').then((m) => m.AccountsPage),
      },
      {
        path: 'comptes/:id',
        title: 'Compte — Cashflow',
        loadComponent: () =>
          import('./features/accounts/account-detail').then((m) => m.AccountDetailPage),
      },
      {
        path: 'biens',
        title: 'Biens — Cashflow',
        loadComponent: () => import('./features/assets/assets').then((m) => m.AssetsPage),
      },
      {
        path: 'documents',
        title: 'Documents — Cashflow',
        loadComponent: () => import('./features/documents/documents').then((m) => m.DocumentsPage),
      },
      {
        path: 'documents/:id',
        title: 'Document — Cashflow',
        loadComponent: () =>
          import('./features/documents/document-page').then((m) => m.DocumentPage),
      },
      {
        path: 'biens/:id',
        title: 'Bien — Cashflow',
        loadComponent: () => import('./features/assets/assets').then((m) => m.AssetPage),
      },
      {
        path: 'prets',
        title: 'Prêts — Cashflow',
        loadComponent: () => import('./features/loans/loans').then((m) => m.LoansPage),
      },
      {
        path: 'prets/capacite',
        title: "Capacité d'achat — Cashflow",
        loadComponent: () =>
          import('./features/credit/purchase-capacity').then((m) => m.PurchaseCapacityPage),
      },
      {
        path: 'prets/simulations',
        title: 'Simulations — Cashflow',
        loadComponent: () => import('./features/credit/simulations').then((m) => m.SimulationsPage),
      },
      {
        path: 'prets/demandes',
        title: 'Demandes de crédit — Cashflow',
        loadComponent: () =>
          import('./features/credit/applications').then((m) => m.ApplicationsPage),
      },
      {
        path: 'prets/demandes/:id',
        title: 'Demande de crédit — Cashflow',
        loadComponent: () =>
          import('./features/credit/application-page').then((m) => m.ApplicationPage),
      },
      {
        path: 'prets/:id',
        title: 'Prêt — Cashflow',
        loadComponent: () => import('./features/loans/loans').then((m) => m.LoanPage),
      },
      {
        path: 'salaires',
        title: 'Salaires — Cashflow',
        loadComponent: () => import('./features/salaries/salaries').then((m) => m.SalariesPage),
      },
      {
        path: 'salaires/:id',
        title: 'Emploi — Cashflow',
        loadComponent: () =>
          import('./features/salaries/employment-page').then((m) => m.EmploymentPage),
      },
      {
        path: 'transactions/:id',
        title: 'Transaction — Cashflow',
        loadComponent: () =>
          import('./features/transactions/transaction-page').then((m) => m.TransactionPage),
      },
      {
        path: 'transactions',
        title: 'Transactions — Cashflow',
        loadComponent: () =>
          import('./features/transactions/transactions').then((m) => m.TransactionsPage),
      },
      {
        path: 'import',
        title: 'Import — Cashflow',
        loadComponent: () =>
          import('./features/import/import-history').then((m) => m.ImportHistoryPage),
      },
      {
        path: 'import/nouveau',
        title: 'Nouvel import — Cashflow',
        loadComponent: () =>
          import('./features/import/import-wizard').then((m) => m.ImportWizardPage),
      },
      {
        path: 'import/:id',
        title: 'Revue de l’import — Cashflow',
        loadComponent: () =>
          import('./features/import/import-review').then((m) => m.ImportReviewPage),
      },
      {
        path: 'budget',
        title: 'Budget — Cashflow',
        loadComponent: () => import('./features/budget/budget').then((m) => m.BudgetPage),
      },
      {
        path: 'budget/assistant',
        title: 'Assistant de budget — Cashflow',
        loadComponent: () =>
          import('./features/budget/assistant').then((m) => m.BudgetAssistantPage),
      },
      {
        path: 'budget/projets',
        title: 'Projets — Cashflow',
        loadComponent: () => import('./features/budget/projects').then((m) => m.ProjectsPage),
      },
      {
        path: 'budget/projets/:id',
        title: 'Projet — Cashflow',
        loadComponent: () => import('./features/budget/projects').then((m) => m.ProjectPage),
      },
      {
        path: 'projections',
        title: 'Projections — Cashflow',
        loadComponent: () =>
          import('./features/projections/projections').then((m) => m.ProjectionsPage),
      },
      {
        path: 'recurrences',
        title: 'Récurrences — Cashflow',
        loadComponent: () =>
          import('./features/projections/recurrences').then((m) => m.RecurrencesPage),
      },
      {
        path: 'analyses',
        title: 'Analyses — Cashflow',
        loadComponent: () => import('./features/analyses/analyses').then((m) => m.AnalysesPage),
      },
      {
        path: 'reglages/regles',
        title: 'Règles — Cashflow',
        loadComponent: () => import('./features/settings/rules').then((m) => m.RulesPage),
      },
      {
        path: 'reglages/baremes',
        title: 'Barèmes de taux — Cashflow',
        loadComponent: () => import('./features/savings/schedules').then((m) => m.SchedulesPage),
      },
      {
        path: 'reglages/scenarios',
        title: 'Scénarios — Cashflow',
        loadComponent: () =>
          import('./features/projections/scenarios').then((m) => m.ScenariosPage),
      },
      {
        path: 'reglages/profils',
        title: 'Profils d’import — Cashflow',
        loadComponent: () => import('./features/settings/profiles').then((m) => m.ProfilesPage),
      },
      {
        path: 'reglages',
        title: 'Réglages — Cashflow',
        loadComponent: () => import('./features/settings/settings').then((m) => m.SettingsPage),
      },
      {
        path: 'reglages/tiers',
        title: 'Tiers — Cashflow',
        loadComponent: () => import('./features/payees/payees').then((m) => m.PayeesPage),
      },
      {
        path: 'tiers/groupe/:id',
        title: 'Groupe de tiers — Cashflow',
        data: { kind: 'group' },
        loadComponent: () => import('./features/payees/payee-page').then((m) => m.PayeePage),
      },
      {
        path: 'tiers/:id',
        title: 'Tiers — Cashflow',
        data: { kind: 'payee' },
        loadComponent: () => import('./features/payees/payee-page').then((m) => m.PayeePage),
      },
      {
        path: 'reglages/categories',
        title: 'Catégories — Cashflow',
        loadComponent: () => import('./features/settings/categories').then((m) => m.CategoriesPage),
      },
      {
        path: 'reglages/etablissements',
        title: 'Établissements — Cashflow',
        loadComponent: () =>
          import('./features/settings/institutions').then((m) => m.InstitutionsPage),
      },
    ],
  },
  { path: '**', redirectTo: '' },
];
