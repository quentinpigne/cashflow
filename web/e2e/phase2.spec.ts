import { expect, test } from '@playwright/test';
import path from 'node:path';

import { isMobile, login, openAccount, pickCategory } from './helpers';

const FIXTURE = path.resolve(__dirname, '../../fixtures/demo/releve-courant.csv');

test.describe.configure({ mode: 'serial' });

test('E10–E11 : importer un relevé avec un profil, pointer et appliquer une règle (IMP-01 à 06)', async ({
  page,
}) => {
  test.skip(
    isMobile(page),
    'L’import est conçu pour l’ordinateur ; une seule exécution suffit (données partagées).',
  );
  await login(page);
  await page.goto('/import/nouveau');
  const account = page.getByRole('combobox', { name: 'Compte' });
  await expect(account).toContainText(/\w/);
  await account.click();
  await page.getByRole('option', { name: 'Compte courant' }).click();
  await page.locator('input[type=file]').setInputFiles(FIXTURE);
  await page.getByRole('combobox', { name: "Profil d'import" }).click();
  await page.getByRole('option', { name: 'Banque démo (CSV)' }).click();
  await page.getByRole('button', { name: 'Continuer' }).first().click();
  await expect(page.getByText('4 ligne(s) lue(s)')).toBeVisible();
  await page.getByRole('button', { name: 'Lancer la revue' }).click();

  await expect(page).toHaveURL(/\/import\/[0-9a-f-]{36}$/);
  await expect(page.getByText('À pointer (1)')).toBeVisible();
  await expect(page.getByText(/Pointe « LOYER APPARTEMENT »/)).toBeVisible();
  await expect(page.locator('.tag', { hasText: 'règle' })).toBeVisible();
  await page.getByRole('button', { name: /^Valider/ }).click();
  await expect(page.getByText('Import validé')).toBeVisible();
  await expect(page.getByText(/3 transaction\(s\) créée\(s\), 1 pointée\(s\)/)).toBeVisible();

  await page.goto('/import');
  await expect(page.getByText('releve-courant.csv')).toBeVisible();
});

test('E13 : saisir un point de contrôle à écart nul et rapprocher (RAP-01, RAP-03)', async ({
  page,
}) => {
  await login(page);
  // Un compte qu'aucun autre test ne modifie : un rapprochement verrouille sa période (R-RAP-7).
  await openAccount(page, isMobile(page) ? 'LDDS' : 'Espèces');
  const balance = await page.locator('.balance strong.amount').textContent();
  const panel = page.locator('app-reconciliation-panel');
  await panel.getByRole('button', { name: /Point de contrôle/ }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Solde').fill((balance ?? '').replace(/[^\d,]/g, ''));
  await dialog.getByRole('button', { name: /Ajouter|Remplacer/ }).click();
  await expect(panel.getByText('écart nul').first()).toBeVisible();
  await panel.getByRole('button', { name: 'Rapprocher' }).first().click();
  await page.getByRole('dialog').getByRole('button', { name: 'Rapprocher' }).click();
  await expect(panel.locator('mat-icon', { hasText: 'lock' }).first()).toBeVisible();
});

test('E14 : catégoriser plusieurs transactions d’un coup (TRX-06, TRX-08)', async ({ page }) => {
  await login(page);
  await page.goto('/transactions?a_categoriser=1');
  await expect(page.locator('.list-head')).toContainText(/transaction\(s\)/);
  const rows = page.locator('.row mat-checkbox:not(.mat-mdc-checkbox-disabled)');
  const count = await rows.count();
  test.skip(count === 0, 'plus rien à catégoriser');
  await rows.first().click();
  const bulk = page.locator('.bulk');
  await pickCategory(page, bulk, 'Supermarché', 'Supermarché / épicerie');
  await bulk.getByRole('button', { name: 'Catégoriser' }).click();
  await expect(page.getByText(/transaction\(s\) catégorisée\(s\)/)).toBeVisible();
});

test('E15 : analyses par catégorie et évolution (TDB-02, TDB-03)', async ({ page }) => {
  await login(page);
  await page.goto('/analyses');
  await expect(page.getByRole('cell', { name: /Alimentation/ }).first()).toBeVisible();
  await expect(page.locator('[echarts] canvas').first()).toBeVisible();
  await page.getByRole('tab', { name: 'Évolution' }).click();
  await expect(page.getByRole('columnheader', { name: 'Taux 12 mois' })).toBeVisible();
});

test('E16 : la règle de démonstration est listée (IMP-08)', async ({ page }) => {
  await login(page);
  await page.goto('/reglages/regles');
  await expect(page.getByText('Netflix', { exact: true })).toBeVisible();
});
