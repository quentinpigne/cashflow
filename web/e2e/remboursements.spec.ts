import { expect, test } from '@playwright/test';

import { isMobile, login, openedDialog } from './helpers';

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
  const dialog = await openedDialog(page);
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

test('E7 : « Lier une dépense » liste toutes les dépenses, par pages, et les cherche (R-TRX-54)', async ({
  page,
}) => {
  await login(page);
  // Une entrée d'argent à affecter, supprimée à la fin.
  const accounts = await (await page.request.get('/api/v1/accounts')).json();
  const checking = accounts.find((a: { name: string }) => a.name === 'Compte courant');
  const created = await page.request.post('/api/v1/transactions', {
    data: { account_id: checking.id, date: '2026-10-18', amount_minor: 5_000, label: 'AVOIR' },
  });
  const { id } = await created.json();
  await page.goto(`/transactions/${id}`);
  await page.getByRole('button', { name: 'Lier une dépense' }).click();
  const dialog = await openedDialog(page, 'Rembourse une dépense');

  // Dix dépenses par page, et toutes les pages : plus de cinquante dépenses dans la démonstration.
  await expect(dialog.locator('.row')).toHaveCount(10);
  const paginator = dialog.locator('mat-paginator');
  await expect(paginator).toContainText(/1 – 10 sur \d{2,}/);

  // Une dépense cochée en page 2 le reste en revenant en page 1.
  await paginator.getByRole('button', { name: 'Page suivante' }).click();
  await expect(paginator).toContainText(/11 – 20 sur/);
  const second = dialog.locator('.row').first();
  await second.getByRole('checkbox').check();
  await paginator.getByRole('button', { name: 'Page précédente' }).click();
  await expect(paginator).toContainText(/1 – 10 sur/);
  await expect(dialog.locator('mat-chip')).toHaveCount(1);

  // Recherche par libellé, puis par montant.
  await dialog.getByLabel('Rechercher une dépense').fill('dentaire');
  await expect(dialog.locator('.row').first()).toContainText('CABINET DENTAIRE');
  await expect(dialog.locator('.row', { hasNotText: 'CABINET DENTAIRE' })).toHaveCount(0);
  await dialog.getByLabel('Rechercher une dépense').fill('890');
  await expect(dialog.locator('.row').first()).toContainText('LOYER APPARTEMENT');
  await dialog.getByRole('button', { name: 'Annuler' }).click();
  await page.request.delete(`/api/v1/transactions/${id}`);
});

test('E7 : depuis une dépense, « Lier un remboursement » choisit l’entrée qui la rembourse (R-TRX-62)', async ({
  page,
}) => {
  test.skip(isMobile(page), 'données partagées : modifiées une seule fois, sur ordinateur');
  await login(page);
  const accounts = await (await page.request.get('/api/v1/accounts')).json();
  const checking = accounts.find((a: { name: string }) => a.name === 'Compte courant');
  const created = await page.request.post('/api/v1/transactions', {
    data: {
      account_id: checking.id,
      date: '2026-10-18',
      amount_minor: 4_000,
      label: 'AVOIR RESTAURANT',
    },
  });
  const { id } = await created.json();

  await page.goto('/transactions?texte=CARTE%20RESTAURANT');
  await page
    .locator('.row', { hasText: 'CARTE RESTAURANT' })
    .first()
    .getByRole('button')
    .first()
    .click();
  const panel = page.locator('app-refund-panel');
  await expect(panel).toContainText("N'est remboursée par aucune entrée.");
  await panel.getByRole('button', { name: 'Lier un remboursement' }).click();
  const dialog = await openedDialog(page, 'Remboursée par');
  await dialog.getByLabel('Rechercher une entrée').fill('avoir');
  await expect(dialog.locator('.row')).toHaveCount(1);
  await dialog.getByRole('checkbox', { name: 'Rembourse AVOIR RESTAURANT' }).check();
  await dialog.getByLabel('Montant remboursé de AVOIR RESTAURANT').fill('10');
  await dialog.getByRole('button', { name: 'Ventiler' }).click();
  await expect(dialog).toBeHidden();
  await expect(panel).toContainText(/AVOIR RESTAURANT · 18 oct\.\s*10,00\s€/);

  // L'entrée est ventilée : 10 € remboursent la dépense, 30 € restent à affecter.
  await page.goto(`/transactions/${id}`);
  await expect(page.locator('app-refund-panel')).toContainText('CARTE RESTAURANT');
  await page.request.delete(`/api/v1/transactions/${id}`);
});
