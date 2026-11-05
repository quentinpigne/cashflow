import { expect, test } from '@playwright/test';

import { login } from './helpers';

const SHOTS = process.env['E2E_SHOTS'];

test('E15, E18 : choisir un mois lointain par le calendrier, années puis mois', async ({
  page,
}) => {
  await login(page);
  await page.goto('/analyses');
  // Le jeu de démonstration est figé au 20 octobre 2026.
  await page.getByRole('button', { name: /^Choisir le mois, octobre 2026/i }).click();
  const calendar = page.locator('mat-calendar');
  await expect(calendar).toBeVisible();
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/period-years.png` });
  await calendar.getByRole('button', { name: '2025' }).click();
  await calendar.getByRole('button', { name: /mars 2025/i }).click();
  await expect(calendar).toBeHidden();
  await expect(page.getByRole('button', { name: /^Choisir le mois, mars 2025/i })).toBeVisible();
  // Les flèches restent là pour un mois à la fois.
  await page.getByRole('button', { name: 'Mois suivant' }).first().click();
  await expect(page.getByRole('button', { name: /^Choisir le mois, avril 2025/i })).toBeVisible();

  // Pas de mois futur dans les analyses.
  await page.getByRole('button', { name: /^Choisir le mois/ }).click();
  await calendar.getByRole('button', { name: '2026' }).click();
  await expect(calendar.getByRole('button', { name: /novembre 2026/i })).toBeDisabled();
  await page.keyboard.press('Escape');

  // Le budget, lui, va dans le futur, et garde le mois dans l'adresse.
  await page.goto('/budget');
  await page.getByRole('button', { name: /^Choisir le mois/ }).click();
  await calendar.getByRole('button', { name: '2027' }).click();
  await calendar.getByRole('button', { name: /janvier 2027/i }).click();
  await expect(page).toHaveURL(/mois=2027-01/);

  // Par tiers : l'année se choisit sans descendre aux mois.
  await page.goto('/analyses');
  await page.getByRole('tab', { name: 'Par tiers' }).click();
  const payees = page.locator('app-payee-analysis');
  await payees.getByRole('radio', { name: 'Année' }).click();
  await payees.getByRole('button', { name: /^Choisir l’année, 2026/ }).click();
  await calendar.getByRole('button', { name: '2024' }).click();
  await expect(calendar).toBeHidden();
  await expect(payees.getByRole('button', { name: /^Choisir l’année, 2024/ })).toBeVisible();
});
