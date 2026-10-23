import { Page, expect, test } from '@playwright/test';

import { login, openAccount, openQuickEntry } from './helpers';

const unique = () => Math.random().toString(36).slice(2, 7).toUpperCase();

/** Choisit (ou crée) un tiers dans un sélecteur à autocomplétion. */
async function pickPayee(page: Page, search: string, option: string | RegExp): Promise<void> {
  const input = page.locator('app-payee-picker input').first();
  await input.focus();
  await input.fill(search);
  await page.getByRole('option', { name: option }).first().click();
}

test('E31, E32 : tiers par groupe et fiche d’un groupe (TIE-01, TIE-04)', async ({ page }) => {
  await login(page);
  await page.goto('/reglages');
  await page.getByRole('link', { name: 'Tiers' }).click();
  await expect(page.getByRole('heading', { name: 'Tiers', level: 1 })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Carrefour Meylan' })).toBeVisible();
  await expect(page.getByText('Sans groupe')).toBeVisible();

  await page.getByRole('link', { name: 'Carrefour', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Carrefour', level: 1 })).toBeVisible();
  await expect(page.getByText(/Groupe de 2 tiers/)).toBeVisible();
  await expect(page.getByRole('heading', { name: '12 derniers mois' })).toBeVisible();
  await expect(page.locator('[echarts] canvas').first()).toBeVisible();
  await expect(
    page.getByText('Alimentation & restauration › Supermarché / épicerie', { exact: true }),
  ).toBeVisible();

  await page.getByRole('link', { name: 'Tout voir' }).click();
  await expect(page).toHaveURL(/groupe=/);
  await expect(page.getByText(/Carrefour (Meylan|Market Grenoble) ·/).first()).toBeVisible();
});

test('E5 : un tiers choisi propose la catégorie de sa dernière transaction (R-TIE-14)', async ({
  page,
}) => {
  await login(page);
  const label = `TEST TIERS ${unique()}`;
  await openAccount(page, 'Compte courant');
  await openQuickEntry(page);
  await page.getByLabel('Montant').fill('42,50');
  await pickPayee(page, 'Meylan', 'Carrefour Meylan');
  await expect(page.locator('app-category-picker input')).toHaveValue(/Supermarché \/ épicerie$/);
  await page.getByLabel('Libellé').fill(label);
  await page.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Transaction enregistrée')).toBeVisible();
  await page.getByRole('button', { name: 'Voir', exact: true }).click();
  await expect(page.locator('app-payee-picker input')).toHaveValue('Carrefour Meylan');
});

test('E5 : créer un tiers depuis la saisie (R-TIE-15)', async ({ page }) => {
  await login(page);
  const name = `Boutique ${unique()}`;
  await openAccount(page, 'Compte courant');
  await openQuickEntry(page);
  await page.getByLabel('Montant').fill('9,90');
  await pickPayee(page, name, `Créer le tiers « ${name} »`);
  await expect(page.locator('app-payee-picker input')).toHaveValue(name);
  await page.getByLabel('Libellé').fill(name.toUpperCase());
  await page.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Transaction enregistrée')).toBeVisible();
  await page.goto('/reglages/tiers');
  await expect(page.getByRole('link', { name })).toBeVisible();
});

test('E15 : dépenses par groupe de tiers (TIE-04)', async ({ page }) => {
  await login(page);
  await page.goto('/analyses');
  await page.getByRole('tab', { name: 'Par tiers' }).click();
  const row = page.getByRole('row', { name: /Carrefour/ }).first();
  await expect(row).toBeVisible();
  await row.getByRole('button', { name: 'Déplier Carrefour' }).click();
  await expect(page.getByRole('button', { name: 'Carrefour Meylan' })).toBeVisible();
  await page.getByRole('button', { name: 'Carrefour Meylan' }).click();
  await expect(page.getByRole('heading', { name: 'Carrefour Meylan', level: 1 })).toBeVisible();
});
