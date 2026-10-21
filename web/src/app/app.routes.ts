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
