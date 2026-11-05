import { expect, test } from '@playwright/test';

import { isMobile, login, openedDialog, pickCategory } from './helpers';

test('E14, E6 : ventiler une sélection en pourcentages (TRX-14, R-TRX-16)', async ({ page }) => {
  test.skip(isMobile(page), 'données partagées : modifiées une seule fois, sur ordinateur');
  await login(page);
  await page.goto('/transactions?texte=CARTE%20SUPERMARCHE');
  const rows = page.locator('.row', { hasText: 'CARTE SUPERMARCHE' });
  await expect(rows.first()).toBeVisible();
  // Deux montants différents : la saisie se fait en pourcentages seulement.
  await rows.nth(0).locator('mat-checkbox').click();
  await rows.nth(1).locator('mat-checkbox').click();
  await page.locator('.bulk').getByRole('button', { name: 'Ventiler' }).click();
  const dialog = await openedDialog(page, /Ventiler 2 transaction/);
  await expect(dialog.getByText(/Les montants diffèrent/)).toBeVisible();
  await expect(dialog.getByRole('radiogroup', { name: 'Mode de saisie' })).toHaveCount(0);

  const lines = dialog.locator('.line');
  await pickCategory(page, lines.nth(0), 'Supermarché', 'Supermarché / épicerie');
  await lines.nth(0).getByLabel('%').fill('60');
  await dialog.getByRole('button', { name: 'Ajouter une ligne' }).click();
  await pickCategory(page, lines.nth(1), 'Restaurants', 'Restaurants');
  await expect(dialog.getByRole('button', { name: 'Valider' })).toBeDisabled();
  await lines.nth(1).getByLabel('%').fill('40');
  await dialog.getByRole('button', { name: 'Valider' }).click();

  await expect(page.getByText('2 transaction(s) ventilée(s)')).toBeVisible();
  await expect(rows.nth(0)).toContainText('Ventilée');
  await expect(rows.nth(1)).toContainText('Ventilée');
});

test('E14, E6 : des montants égaux se ventilent aussi en montants (TRX-14)', async ({ page }) => {
  test.skip(isMobile(page), 'données partagées : modifiées une seule fois, sur ordinateur');
  await login(page);
  await page.goto('/transactions?texte=BOX%20INTERNET');
  const rows = page.locator('.row', { hasText: 'BOX INTERNET' });
  await rows.nth(0).locator('mat-checkbox').click();
  await rows.nth(1).locator('mat-checkbox').click();
  await page.locator('.bulk').getByRole('button', { name: 'Ventiler' }).click();
  const dialog = await openedDialog(page, /Ventiler 2 transaction/);
  await expect(dialog.getByText(/Les montants diffèrent/)).toHaveCount(0);
  const lines = dialog.locator('.line');
  await lines.nth(0).getByLabel('Montant').fill('20');
  await dialog.getByRole('button', { name: 'Ajouter une ligne' }).click();
  await expect(lines.nth(1).getByLabel('Montant')).toHaveValue('9,99');
  await pickCategory(page, lines.nth(1), 'Téléphone', 'Téléphone');
  await dialog.getByRole('button', { name: 'Valider' }).click();
  await expect(page.getByText('2 transaction(s) ventilée(s)')).toBeVisible();
});
