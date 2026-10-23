import { expect, test } from '@playwright/test';
import path from 'node:path';

import { isMobile, login } from './helpers';

const FIXTURE = path.resolve(__dirname, '../../fixtures/lcl/releve-fictif.pdf');

test('E10, E11 : importer un relevé PDF LCL, sans profil (IMP-10)', async ({ page }) => {
  test.skip(isMobile(page), 'L’import est conçu pour l’ordinateur ; une seule exécution suffit.');
  await login(page);
  await page.goto('/import/nouveau');
  const account = page.getByRole('combobox', { name: 'Compte' });
  await expect(account).toContainText(/\w/);
  await account.click();
  await page.getByRole('option', { name: 'Espèces' }).click();
  await page.locator('input[type=file]').setInputFiles(FIXTURE);
  await expect(page.getByText(/Relevé PDF LCL : lu directement, sans profil/)).toBeVisible();
  await page.getByRole('button', { name: 'Continuer' }).first().click();
  await expect(page.getByText('7 ligne(s) lue(s)')).toBeVisible();
  await page.getByRole('button', { name: 'Lancer la revue' }).click();

  await expect(page).toHaveURL(/\/import\/[0-9a-f-]{36}$/);
  await expect(
    page.getByText(/Contrôle du relevé : soldes de début et de fin cohérents/),
  ).toBeVisible();
  await expect(page.getByText('achat le 3 déc.')).toBeVisible();
  await page.getByRole('button', { name: /^Valider/ }).click();
  await expect(page.getByText(/7 transaction\(s\) créée\(s\)/)).toBeVisible();
});
