import { expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

test('E14, E7 : la flèche de retour revient à la liste, avec ses filtres (TRX-08)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/transactions');
  if (isMobile(page)) await page.getByRole('button', { name: 'Filtres' }).click();
  await page.getByLabel('Texte').fill('SALAIRE');
  await expect(page).toHaveURL(/texte=SALAIRE/);
  await page
    .getByRole('button', { name: /^VIREMENT SALAIRE/ })
    .first()
    .click();
  await expect(page).toHaveURL(/\/transactions\/[0-9a-f-]+$/);

  await page.getByRole('link', { name: 'Retour' }).click();
  await expect(page).toHaveURL(/\/transactions\?texte=SALAIRE/);
  if (isMobile(page)) await page.getByRole('button', { name: 'Filtres' }).click();
  await expect(page.getByLabel('Texte')).toHaveValue('SALAIRE');
  await expect(page.getByRole('button', { name: /^VIREMENT SALAIRE/ }).first()).toBeVisible();
});

test('E7 : ouverte directement, une transaction revient à son compte', async ({ page }) => {
  await login(page);
  await page.goto('/transactions?texte=SALAIRE');
  await page
    .getByRole('button', { name: /^VIREMENT SALAIRE/ })
    .first()
    .click();
  await expect(page).toHaveURL(/\/transactions\/[0-9a-f-]+$/);
  // Un nouvel onglet : l'historique ne contient rien de l'application.
  const url = page.url();
  const other = await page.context().newPage();
  await other.goto(url);
  await other.getByRole('link', { name: 'Retour' }).click();
  await expect(other).toHaveURL(/\/comptes\/[0-9a-f-]+/);
});
