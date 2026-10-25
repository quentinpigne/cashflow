import { expect, test } from '@playwright/test';

import {
  TODAY,
  isMobile,
  login,
  openAccount,
  openQuickEntry,
  pickCategory,
  transactionRow,
} from './helpers';

const unique = () => Math.random().toString(36).slice(2, 7).toUpperCase();

test('E0 : un mauvais mot de passe est refusé, le bon ouvre l’accueil', async ({ page }) => {
  await page.clock.install({ time: new Date(TODAY) });
  await page.goto('/');
  await expect(page).toHaveURL(/connexion/);
  await page.getByLabel('Mot de passe').fill('mauvais');
  await page.getByRole('button', { name: 'Se connecter' }).click();
  await expect(page.getByText('Mot de passe incorrect.')).toBeVisible();
  await login(page);
  await expect(page.getByText('mois en cours')).toBeVisible();
});

test('E5 : saisir une dépense catégorisée (TRX-01)', async ({ page }) => {
  await login(page);
  const label = `TEST DEPENSE ${unique()}`;
  await openAccount(page, 'Compte courant');
  await openQuickEntry(page);
  await page.getByLabel('Montant').fill('12,50');
  await pickCategory(page, page.locator('app-quick-entry'), 'Restaurants', 'Restaurants');
  await page.getByLabel('Libellé').fill(label);
  await page.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Transaction enregistrée')).toBeVisible();
  const row = transactionRow(page, label);
  await expect(row).toBeVisible();
  await expect(row).toContainText('−12,50');
  if (!isMobile(page)) {
    await expect(row).toContainText('Restaurants');
  }
});

test('E6 : ventiler 50/30/20 en pourcentages (TRX-03)', async ({ page }) => {
  await login(page);
  const label = `TEST VENTILEE ${unique()}`;
  await openAccount(page, 'Compte courant');
  await openQuickEntry(page);
  await page.getByLabel('Montant').fill('120');
  await page.getByLabel('Libellé').fill(label);
  await page.getByRole('button', { name: 'Ventiler' }).click();
  const editor = page.locator('app-split-editor');
  await editor.getByRole('radio', { name: '%' }).click();
  await editor.getByRole('button', { name: 'Ajouter une ligne' }).click();
  await editor.getByRole('button', { name: 'Ajouter une ligne' }).click();
  const values = editor.getByLabel('%');
  await values.nth(0).fill('50');
  await values.nth(1).fill('30');
  await values.nth(2).fill('20');
  await expect(editor.getByText('Reste à ventiler')).toContainText('0,00');
  await editor.getByRole('button', { name: 'Valider' }).click();
  await expect(page.getByText('Ventilée en 3 lignes')).toBeVisible();
  await page.getByRole('button', { name: 'Enregistrer' }).click();
  const row = transactionRow(page, label);
  await expect(row).toContainText('Ventilée');
  await row.click();
  await expect(page.getByText('−60,00')).toBeVisible();
  await expect(page.getByText('−36,00')).toBeVisible();
  await expect(page.getByText('−24,00')).toBeVisible();
});

test('E5 : un virement crée les deux jambes (TRX-05)', async ({ page }) => {
  await login(page);
  const label = `TEST VIREMENT ${unique()}`;
  await openAccount(page, 'Compte courant');
  await openQuickEntry(page);
  // Sur mobile, un clic pendant l'ouverture animée du dialogue peut se perdre : on vérifie qu'il a pris.
  const transfer = page.getByRole('radio', { name: 'Virement' });
  await expect(async () => {
    await transfer.click();
    await expect(transfer).toBeChecked({ timeout: 1000 });
  }).toPass();
  await page.getByLabel('Montant').fill('50');
  await page.getByRole('combobox', { name: "Compte d'arrivée" }).click();
  await page.getByRole('option', { name: 'Livret A' }).click();
  await page.getByLabel('Libellé').fill(label);
  await page.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Virement enregistré')).toBeVisible();
  await expect(transactionRow(page, label)).toContainText('−50,00');
  await openAccount(page, 'Livret A');
  await expect(transactionRow(page, label)).toContainText('50,00');
});

test('E1 : catégoriser sur place depuis l’accueil (R-TDB-5)', async ({ page }) => {
  await login(page);
  const card = page.locator('section.uncategorized');
  const title = card.getByRole('heading');
  const before = Number((await title.textContent())?.match(/\((\d+)\)/)?.[1]);
  const first = card.locator('.todo').first();
  // La ligne disparaît dès qu'elle est catégorisée : pas de texte à vérifier.
  await pickCategory(page, first, 'Supermarché', 'Supermarché / épicerie', false);
  if (before > 1) {
    await expect(title).toHaveText(`À catégoriser (${before - 1})`);
  } else {
    await expect(card).toBeHidden();
  }
});

test('E4 : créer, clôturer puis supprimer un compte (CPT-01, CPT-04, R-CPT-7)', async ({
  page,
}) => {
  await login(page);
  const name = `Livret test ${unique()}`;
  await page.goto('/comptes');
  await page.getByRole('button', { name: /Nouveau compte/ }).click();
  await page.getByRole('combobox', { name: 'Nature' }).click();
  await page.getByRole('option', { name: 'Livret A' }).click();
  await page.getByLabel('Nom', { exact: true }).fill(name);
  await page.getByRole('button', { name: 'Créer le compte' }).click();
  await expect(page.getByRole('heading', { name })).toBeVisible();

  await page.getByRole('button', { name: 'Actions sur le compte' }).click();
  await page.getByRole('menuitem', { name: /Clôturer/ }).click();
  await page.getByRole('button', { name: 'Clôturer', exact: true }).click();
  await expect(page.getByText(/Clôturé le/)).toBeVisible();

  await page.getByRole('button', { name: 'Actions sur le compte' }).click();
  await page.getByRole('menuitem', { name: /Supprimer/ }).click();
  await page.getByRole('button', { name: 'Supprimer' }).click();
  await expect(page).toHaveURL(/\/comptes$/);
  await expect(page.getByText(name)).toHaveCount(0);
});

test('E2, E4 : colorer un compte et le remonter en tête de sa famille (CPT-07)', async ({
  page,
}) => {
  await login(page);
  const name = `Zz coffre ${unique()}`;
  await page.goto('/comptes');
  await page.getByRole('button', { name: /Nouveau compte/ }).click();
  await page.getByRole('combobox', { name: 'Nature' }).click();
  await page.getByRole('option', { name: 'Espèces' }).click();
  await page.getByLabel('Nom', { exact: true }).fill(name);
  await page.getByRole('radio', { name: 'Violet' }).click();
  await page.getByRole('button', { name: 'Créer le compte' }).click();
  const heading = page.getByRole('heading', { name });
  await expect(heading.locator('.account-dot')).toHaveCSS('background-color', 'rgb(142, 36, 170)');

  await page.goto('/comptes');
  const group = page.locator('section.group', { hasText: 'Liquidités' });
  await expect(group.locator('a.row').last()).toContainText(name);
  if (!isMobile(page)) {
    await page.getByRole('button', { name: 'Ordonner' }).click();
    const handle = group.locator('.row.ordering', { hasText: name }).locator('.handle');
    const target = group.locator('.row.ordering').first();
    const from = await handle.boundingBox();
    const to = await target.boundingBox();
    if (!from || !to) throw new Error('ligne introuvable');
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
    await page.mouse.down();
    for (let i = 1; i <= 15; i++) {
      await page.mouse.move(from.x + from.width / 2, from.y + ((to.y + 2 - from.y) * i) / 15);
    }
    await page.mouse.up();
    await page.getByRole('button', { name: 'Terminé' }).click();
    await page.reload();
    await expect(group.locator('a.row').first()).toContainText(name);
  }

  await group.locator('a.row', { hasText: name }).click();
  await page.getByRole('button', { name: 'Actions sur le compte' }).click();
  await page.getByRole('menuitem', { name: /Supprimer/ }).click();
  await page.getByRole('button', { name: 'Supprimer' }).click();
  await expect(page).toHaveURL(/\/comptes$/);
});

test('E8 : créer, renommer et supprimer une catégorie (TRX-04)', async ({ page }) => {
  await login(page);
  const name = `Catégorie ${unique()}`;
  await page.goto('/reglages/categories');
  await page.getByRole('button', { name: /Ajouter/ }).click();
  await page.getByRole('dialog').getByLabel('Nom').fill(name);
  await page.getByRole('button', { name: 'Créer' }).click();
  await expect(page.getByText(name, { exact: true })).toBeVisible();

  await page.getByRole('button', { name: `Actions sur ${name}` }).click();
  await page.getByRole('menuitem', { name: /Renommer/ }).click();
  await page.getByRole('dialog').getByLabel('Nom').fill(`${name} bis`);
  await page.getByRole('button', { name: 'Renommer' }).click();
  await expect(page.getByText(`${name} bis`, { exact: true })).toBeVisible();

  await page.getByRole('button', { name: `Actions sur ${name} bis` }).click();
  await page.getByRole('menuitem', { name: /Supprimer/ }).click();
  await page.getByRole('button', { name: 'Supprimer' }).click();
  await expect(page.getByText(`${name} bis`, { exact: true })).toHaveCount(0);
});
