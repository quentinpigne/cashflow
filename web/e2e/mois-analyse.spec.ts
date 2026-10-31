import { expect, test } from '@playwright/test';

import { isMobile, login, openedDialog } from './helpers';

test('E7, E14 : mois d’analyse d’une transaction et liste filtrée par mois (TRX-13, R-TRX-57, R-TRX-58)', async ({
  page,
}) => {
  test.skip(isMobile(page), 'données partagées : modifié une seule fois, sur ordinateur');
  test.slow();
  await login(page);
  await page.goto('/transactions');
  await page.getByLabel('Texte').fill('SALAIRE');
  // Le salaire d'octobre, versé le 1er : il comptera sur septembre.
  // Le plus récent d'abord : 1er octobre 2026 (un an plus tôt, le 1er octobre 2025 suit).
  await page
    .getByRole('button', { name: /^VIREMENT SALAIRE 1 oct\./ })
    .first()
    .click();
  await page.getByRole('combobox', { name: "Mois d'analyse" }).click();
  await page.getByRole('option', { name: 'Septembre 2026', exact: true }).click();
  await page.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Transaction enregistrée')).toBeVisible();

  // Une règle créée depuis cette transaction reprend le décalage du mois (R-IMP-20, R-IMP-29).
  await page.getByRole('button', { name: "Plus d'actions" }).click();
  await page.getByRole('menuitem', { name: /Créer une règle/ }).click();
  const rule = await openedDialog(page);
  await expect(rule.getByRole('combobox', { name: 'Compter sur le mois' })).toContainText(
    'Mois précédent',
  );
  await rule.getByRole('button', { name: 'Annuler' }).click();

  // La liste ouverte sur septembre le montre, quelle que soit sa date (R-TRX-58).
  await page.goto('/transactions?mois=2026-09');
  await expect(
    page.getByText('Comptent en septembre 2026, quelle que soit leur date'),
  ).toBeVisible();
  const salary = page.getByRole('button', { name: /^VIREMENT SALAIRE 1 oct\./ });
  await expect(salary).toContainText('compte en septembre 2026');

  // Retour au mois de la date.
  await salary.click();
  await page.getByRole('combobox', { name: "Mois d'analyse" }).click();
  await page.getByRole('option', { name: 'Mois de la date' }).click();
  await page.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Transaction enregistrée')).toBeVisible();
  await page.goto('/transactions?mois=2026-09');
  await expect(
    page.locator('.row', { hasText: '1 oct.' }).filter({ hasText: /SALAIRE/i }),
  ).toHaveCount(0);
});

test('E14 : sur mobile, les filtres se replient derrière « Filtres »', async ({ page }) => {
  test.skip(!isMobile(page), 'repli propre au mobile');
  await login(page);
  await page.goto('/transactions');
  await expect(page.getByLabel('Texte')).toHaveCount(0);
  await page.getByRole('button', { name: 'Filtres' }).click();
  await expect(page.getByLabel('Texte')).toBeVisible();
  await page.getByRole('button', { name: 'Filtres' }).click();
  await expect(page.getByLabel('Texte')).toHaveCount(0);
});
