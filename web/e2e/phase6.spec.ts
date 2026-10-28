import { expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

test.describe.configure({ mode: 'serial' });

test('E23 : échéancier des récurrences et nouvelle récurrence (TRX-07)', async ({ page }) => {
  await login(page);
  await page.goto('/recurrences');
  await expect(page.getByText('Loyer').first()).toBeVisible();
  await page.getByRole('tab', { name: 'Récurrences' }).click();
  await expect(page.getByRole('button', { name: /Salaire/ })).toBeVisible();
  test.skip(isMobile(page), 'données partagées : créée une seule fois, sur ordinateur');
  await page
    .locator('.page-header')
    .getByRole('button', { name: /Nouvelle/ })
    .click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Libellé').fill('Abonnement salle de sport');
  await dialog.getByLabel('Compte', { exact: true }).click();
  await page.getByRole('option', { name: 'Compte courant' }).click();
  await dialog.getByLabel('Montant', { exact: true }).fill('35');
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByRole('button', { name: /Abonnement salle de sport/ })).toBeVisible();
});

test('E24 : trésorerie projetée du compte courant (PRJ-01)', async ({ page }) => {
  await login(page);
  await page.goto('/projections');
  await page.getByRole('tab', { name: 'Trésorerie' }).click();
  await expect(page.getByText('Solde le plus bas')).toBeVisible();
  await expect(page.getByText('échéance prévue').first()).toBeVisible();
  await expect(page.locator('[echarts] canvas').first()).toBeVisible();
});

test('E24 : patrimoine projeté par scénario (PRJ-05, TDB-04)', async ({ page }) => {
  await login(page);
  await page.goto('/projections');
  await expect(page.getByText('Médian dans 10 ans')).toBeVisible();
  await expect(page.getByText('Prudent dans 10 ans')).toBeVisible();
  await page.getByRole('radio', { name: '20 ans' }).click();
  await expect(page.getByText('Médian dans 20 ans')).toBeVisible();
});

test('E24, E33 : projection d’épargne et scénarios (PRJ-02, PRJ-03, PRJ-04)', async ({ page }) => {
  await login(page);
  await page.goto('/projections');
  await page.getByRole('tab', { name: 'Épargne' }).click();
  await expect(page.getByText(/Hypothèses :/)).toBeVisible();
  await page.getByRole('link', { name: 'Scénarios' }).click();
  await expect(page).toHaveURL(/\/reglages\/scenarios$/);
  await expect(page.getByRole('heading', { name: /Médian/ })).toBeVisible();
  await expect(page.locator('.tag', { hasText: 'par défaut' })).toBeVisible();
});
