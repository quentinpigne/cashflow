import { expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

test.describe.configure({ mode: 'serial' });

/** Ouvre une rubrique par le menu : latéral sur ordinateur, ⋮ sur mobile. */
async function openSection(page: import('@playwright/test').Page, name: 'Biens' | 'Prêts') {
  if (isMobile(page)) {
    await page.getByRole('button', { name: 'Menu' }).click();
    await page.getByRole('menuitem', { name }).click();
  } else {
    await page.locator('mat-sidenav').getByRole('link', { name, exact: true }).click();
  }
  await expect(page.getByRole('heading', { name, level: 1 })).toBeVisible();
}

test('E25, E26 : un bien et sa valeur nette face à son prêt (BIE-01, R-PRE-10)', async ({
  page,
}) => {
  await login(page);
  await openSection(page, 'Biens');
  await page.getByRole('link', { name: /Appartement/ }).click();
  await expect(page.getByRole('heading', { name: 'Appartement' })).toBeVisible();
  await expect(page.getByText('Valeur nette')).toBeVisible();
  await expect(page.getByRole('link', { name: 'Prêt immobilier' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Estimations' })).toBeVisible();
});

test('E27, E28 : tableau d’amortissement et mensualités ventilées (PRE-02, PRE-03)', async ({
  page,
}) => {
  await login(page);
  await openSection(page, 'Prêts');
  await page.getByRole('link', { name: /^Prêt immobilier/ }).click();
  await expect(page.getByRole('heading', { name: 'Prêt immobilier' })).toBeVisible();
  await expect(page.getByText('1 109,20 €').first()).toBeVisible();
  const table = page.locator('table');
  await expect(table.locator('tbody tr')).toHaveCount(12);
  await expect(table.locator('tbody tr.next')).toHaveCount(1);

  // Annuler la ventilation d'une échéance, puis la refaire depuis la proposition.
  const paid = page.getByRole('button', { name: /Échéance \d+ payée/ });
  await paid.first().click();
  await page.getByRole('menuitem', { name: 'Annuler la ventilation' }).click();
  await page.getByRole('button', { name: 'Annuler la ventilation' }).click();
  await expect(page.getByText(/correspondent à des échéances non ventilées/)).toBeVisible();
  await page.getByRole('button', { name: 'Ventiler les mensualités passées' }).click();
  await page.getByRole('button', { name: /Ventiler 1 mensualité/ }).click();
  await expect(page.getByText(/correspondent à des échéances non ventilées/)).toHaveCount(0);

  // La fiche de la mensualité montre l'échéance et sa répartition.
  await paid.first().click();
  await page.getByRole('menuitem', { name: 'Voir la mensualité' }).click();
  await expect(page.getByText(/Échéance n° \d+ du prêt/)).toBeVisible();
  await expect(page.getByText('Remboursement', { exact: true })).toBeVisible();
  await expect(page.getByText('Prêts › Intérêts')).toBeVisible();
});

test('E28 : la mensualité se recalcule pendant la saisie des conditions (R-PRE-3)', async ({
  page,
}) => {
  await login(page);
  await openSection(page, 'Prêts');
  await page.getByRole('link', { name: /^Prêt immobilier/ }).click();
  await page.getByRole('button', { name: 'Actions sur le prêt' }).click();
  await page.getByRole('menuitem', { name: 'Modifier' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.locator('.preview')).toContainText('1 109,20 €');
  await dialog.getByLabel('Durée').fill('180');
  await expect(dialog.locator('.preview')).toContainText('1 381,16 €');
  await dialog.getByRole('button', { name: 'Annuler' }).click();
});

test('E9 : une rubrique ne se désactive qu’une fois vide (R-BIE-7, R-PRE-12)', async ({ page }) => {
  await login(page);
  await page.goto('/reglages');
  await expect(page.getByRole('switch', { name: 'Biens' })).toBeDisabled();
  await expect(page.getByText(/1 bien\(s\) : les supprimer/)).toBeVisible();
  await expect(page.getByRole('switch', { name: 'Prêts' })).toBeDisabled();
});
