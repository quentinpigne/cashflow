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
