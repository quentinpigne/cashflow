import { expect, test } from '@playwright/test';

import { isMobile, login, openAccount, openedDialog } from './helpers';

test.describe.configure({ mode: 'serial' });

test('E3 : intérêts courus d’un Livret A et détail par quinzaine (EPA-02)', async ({ page }) => {
  await login(page);
  await openAccount(page, 'Livret A');
  const panel = page.locator('app-interest-panel');
  await expect(panel.getByText('Courus depuis le 1er janvier')).toBeVisible();
  await expect(panel.getByText(/taux actuel/)).toBeVisible();
  await panel.getByText('Détail par quinzaine').click();
  // 24 quinzaines, plus la ligne d'en-tête.
  await expect(panel.getByRole('row')).toHaveCount(25);
});

test('E3 : inscrire les intérêts de l’année écoulée (EPA-03)', async ({ page }) => {
  test.skip(isMobile(page), 'données partagées : inscrits une seule fois, sur ordinateur');
  await login(page);
  await openAccount(page, 'LDDS');
  const panel = page.locator('app-interest-panel');
  await panel.getByRole('button', { name: 'Année précédente' }).click();
  await panel.getByRole('button', { name: /Inscrire les intérêts 2025/ }).click();
  await expect(page.getByText(/Intérêts 2025 inscrits/)).toBeVisible();
  await expect(panel.getByText('Inscrits')).toBeVisible();
  await expect(panel.getByRole('button', { name: /Inscrire les intérêts/ })).toBeHidden();
});

test('E3 : performance d’une assurance vie et nouvelle valorisation (EPA-05, CPT-06)', async ({
  page,
}) => {
  await login(page);
  await openAccount(page, 'Assurance vie');
  await expect(page.getByText('Capital investi')).toBeVisible();
  await expect(page.getByText('TRI depuis l’ouverture'.replace('’', "'"))).toBeVisible();
  test.skip(isMobile(page), 'données partagées : valorisation saisie sur ordinateur');
  await page.locator('app-valuation-panel').getByRole('button', { name: 'Ajouter' }).click();
  const dialog = await openedDialog(page);
  await dialog.getByLabel('Valeur du compte').fill('15000');
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('20 octobre 2026')).toBeVisible();
  await expect(page.locator('app-valuation-panel').getByText('15 000,00 €').first()).toBeVisible();
});

test('E22 : barèmes de taux (EPA-01)', async ({ page }) => {
  await login(page);
  await page.goto('/reglages/baremes');
  await expect(page.getByRole('heading', { name: 'Livret A (réglementé)' })).toBeVisible();
  await expect(page.getByText(/Utilisé par Livret A/)).toBeVisible();
});

test('accueil : bloc épargne (TDB-05)', async ({ page }) => {
  await login(page);
  await expect(page.getByRole('heading', { name: 'Épargne', exact: true, level: 2 })).toBeVisible();
  await expect(page.getByText('Épargne de précaution').first()).toBeVisible();
  await expect(page.getByText(/TRI global/)).toBeVisible();
});
