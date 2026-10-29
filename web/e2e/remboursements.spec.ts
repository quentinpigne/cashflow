import { expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

test.describe.configure({ mode: 'serial' });

const SHOTS = process.env['E2E_SHOTS'];

test('E1, E7 : remboursement attendu, puis lié depuis l’entrée (TRX-12, R-TRX-53, R-TRX-54)', async ({
  page,
}) => {
  test.skip(isMobile(page), 'données partagées : modifié une seule fois, sur ordinateur');
  await login(page);
  // Le jeu de démonstration attend 63 € pour le dentiste du mois.
  const card = page.locator('section.refunds');
  await expect(card).toContainText('CABINET DENTAIRE');
  if (SHOTS) await card.screenshot({ path: `${SHOTS}/home-refunds.png` });
  await card.getByRole('link', { name: /CABINET DENTAIRE/ }).click();
  const panel = page.locator('app-refund-panel');
  await expect(panel.getByLabel('Remboursement attendu')).toHaveValue('63,00');
  await expect(panel).toContainText(/0,00\s€ remboursés sur 90,00\s€\s+·\s63,00\s€ à recevoir/);
  if (SHOTS) await panel.screenshot({ path: `${SHOTS}/expense-panel.png` });

  // Une entrée de l'assurance maladie, liée depuis sa fiche.
  const accounts = await (await page.request.get('/api/v1/accounts')).json();
  const checking = accounts.find((a: { name: string }) => a.name === 'Compte courant');
  const created = await page.request.post('/api/v1/transactions', {
    data: {
      account_id: checking.id,
      date: '2026-10-19',
      amount_minor: 6_300,
      label: 'REMBOURSEMENT CPAM DENTAIRE',
    },
  });
  const { id } = await created.json();
  await page.goto(`/transactions/${id}`);
  await page.getByRole('button', { name: 'Lier une dépense' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.locator('.row').first()).toContainText('CABINET DENTAIRE');
  await expect(dialog.locator('.row').first()).toContainText('63,00 € attendus');
  await dialog.getByRole('checkbox', { name: 'Rembourse CABINET DENTAIRE' }).check();
  await expect(dialog.getByLabel('Montant remboursé de CABINET DENTAIRE')).toHaveValue('63,00');
  if (SHOTS) await dialog.screenshot({ path: `${SHOTS}/dialog.png` });
  await dialog.getByRole('button', { name: 'Ventiler' }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('app-refund-panel')).toContainText(
    /CABINET DENTAIRE · 8 oct\., −90,00\s€\s*63,00\s€/,
  );
  if (SHOTS)
    await page.locator('app-refund-panel').screenshot({ path: `${SHOTS}/income-panel.png` });

  // L'attente est soldée : l'encadré disparaît de l'accueil.
  await page.goto('/');
  await expect(page.locator('section.net-worth')).toBeVisible();
  await expect(page.locator('section.refunds')).toHaveCount(0);
});
