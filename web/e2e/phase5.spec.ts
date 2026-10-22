import { expect, test } from '@playwright/test';

import { isMobile, login, openAccount } from './helpers';

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
  const dialog = page.getByRole('dialog');
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

test('E2 : un bien compte dans le patrimoine, en face de son prêt (R-CPT-14, R-TDB-1)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/comptes');
  await expect(page.getByText('Biens', { exact: true })).toBeVisible();
  await openAccount(page, 'Appartement');
  await expect(page.getByRole('heading', { name: 'Valeur du bien' })).toBeVisible();
  // Pas de performance pour un bien : elle est propre à l'épargne variable.
  await expect(page.getByRole('heading', { name: 'Performance' })).toHaveCount(0);

  // Le formulaire propose la nature « Bien immobilier » et son libellé de valeur.
  await page.goto('/comptes');
  await page.getByRole('button', { name: /Nouveau compte/ }).click();
  await page.getByRole('combobox', { name: 'Nature' }).click();
  await page.getByRole('option', { name: 'Bien immobilier' }).click();
  await expect(page.getByLabel('Valeur du bien')).toBeVisible();
  await page.getByText('Options', { exact: true }).click();
  await expect(page.getByRole('checkbox', { name: /épargne de précaution/ })).toBeDisabled();
});

test('E3 : tableau d’amortissement d’un prêt et conditions (CPT-09)', async ({ page }) => {
  await login(page);
  await openAccount(page, 'Prêt immobilier');
  const panel = page.locator('app-loan-panel');
  await expect(panel.getByRole('heading', { name: "Tableau d'amortissement" })).toBeVisible();
  await expect(panel.getByText('1 109,20 €').first()).toBeVisible();
  // Une année à la fois : douze échéances, la prochaine mise en évidence.
  await expect(panel.locator('tbody tr')).toHaveCount(12);
  await expect(panel.locator('tbody tr.next')).toHaveCount(1);
  await panel.getByRole('button', { name: 'Année suivante' }).click();
  await expect(panel.locator('tbody tr')).toHaveCount(12);

  // Les conditions se modifient ; la mensualité se recalcule pendant la saisie.
  await panel.getByRole('button', { name: /Conditions/ }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.locator('.preview')).toContainText('1 109,20 €');
  await dialog.getByLabel('Durée').fill('180');
  await expect(dialog.locator('.preview')).toContainText('1 381,16 €');
  await dialog.getByRole('button', { name: 'Annuler' }).click();
});

test('E3 : mensualités ventilées en capital, intérêts et assurance (CPT-10, R-CPT-17)', async ({
  page,
}) => {
  await login(page);
  await openAccount(page, 'Prêt immobilier');
  const panel = page.locator('app-loan-panel');
  // Les mensualités passées de la démo sont ventilées : pas d'écart au tableau.
  await expect(panel.getByText(/écart/)).toHaveCount(0);
  const paid = panel.getByRole('button', { name: /Échéance \d+ payée/ });
  await expect(paid.first()).toBeVisible();

  // Annuler la ventilation d'une échéance, puis la refaire depuis la proposition.
  await paid.first().click();
  await page.getByRole('menuitem', { name: 'Annuler la ventilation' }).click();
  await page.getByRole('button', { name: 'Annuler la ventilation' }).click();
  await expect(panel.getByText(/correspondent à des échéances non ventilées/)).toBeVisible();
  await panel.getByRole('button', { name: 'Ventiler les mensualités passées' }).click();
  await page.getByRole('button', { name: /Ventiler 1 mensualité/ }).click();
  await expect(panel.getByText(/correspondent à des échéances non ventilées/)).toHaveCount(0);

  // La fiche de la mensualité montre l'échéance et sa répartition.
  await paid.first().click();
  await page.getByRole('menuitem', { name: 'Voir la mensualité' }).click();
  await expect(page.getByText(/Échéance n° \d+ du prêt/)).toBeVisible();
  await expect(page.getByText('Capital remboursé')).toBeVisible();
  await expect(page.getByText('Prêts › Intérêts')).toBeVisible();
});
