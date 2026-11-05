import { expect, test } from '@playwright/test';

import { isMobile, login, openedDialog } from './helpers';

test('E14, E7, E15 : adresse d’une sélection, prioritaire sur le tiers dans l’analyse par lieu (TRX-15, R-TRX-60)', async ({
  page,
}) => {
  test.skip(isMobile(page), 'données partagées : modifiées une seule fois, sur ordinateur');
  test.slow();
  await login(page);
  await page.goto('/transactions?texte=CARTE%20RESTAURANT');
  const rows = page.locator('.row', { hasText: 'CARTE RESTAURANT' });
  await rows.nth(0).locator('mat-checkbox').click();
  await rows.nth(1).locator('mat-checkbox').click();
  await page.locator('.bulk').getByRole('button', { name: 'Adresse', exact: true }).click();
  const dialog = await openedDialog(page, /Adresse de 2 transaction/);
  await dialog.getByLabel('Code postal').fill('74290');
  await expect(dialog.getByRole('button', { name: 'Enregistrer' })).toBeDisabled();
  await dialog.getByLabel('Ville').fill('Talloires');
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('2 transaction(s) localisée(s) à Talloires')).toBeVisible();
  await expect(rows.nth(0).getByLabel('Adresse : Talloires')).toBeVisible();

  // La fiche montre l'adresse de la transaction.
  await rows.nth(0).getByRole('button').first().click();
  await expect(page.getByText('74290 Talloires')).toBeVisible();
  await expect(page.getByRole('button', { name: "Modifier l'adresse" })).toBeVisible();

  // L'analyse par lieu compte ces transactions à Talloires, quel que soit leur tiers.
  await page.goto('/analyses');
  await page.getByRole('tab', { name: 'Par lieu' }).click();
  await page.locator('app-location-analysis').getByRole('radio', { name: 'Villes' }).click();
  await page.locator('app-location-analysis').getByRole('radio', { name: '12 mois' }).click();
  await expect(page.getByRole('row', { name: /Talloires/ })).toBeVisible();

  // Retirer l'adresse depuis la fiche.
  await page.goto('/transactions?texte=CARTE%20RESTAURANT');
  await rows.nth(0).getByRole('button').first().click();
  await page.getByRole('button', { name: "Retirer l'adresse" }).click();
  await expect(page.getByText('Adresse retirée')).toBeVisible();
  await expect(page.getByRole('button', { name: "Définir l'adresse" })).toBeVisible();
});
