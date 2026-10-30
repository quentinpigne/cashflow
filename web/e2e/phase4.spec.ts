import { expect, test } from '@playwright/test';

import { isMobile, login, openQuickEntry, pickCategory, openedDialog } from './helpers';

test.describe.configure({ mode: 'serial' });

test('E18 : budget du mois, lignes, hors budget et synthèse (BUD-03)', async ({ page }) => {
  await login(page);
  await page.goto('/budget');
  await expect(page.getByText('Reste à budgéter')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Dépenses mensuelles' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Modifier Loyer' })).toBeVisible();
  await expect(page.getByRole('heading', { name: /Dépenses annuelles/ })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Épargne' })).toBeVisible();
  // Mois précédent : navigation par mois.
  await page.getByRole('button', { name: 'Mois précédent' }).click();
  await expect(page.getByText('Septembre 2026')).toBeVisible();
});

test('E18 : modifier une ligne pour ce mois seulement (R-BUD-51)', async ({ page }) => {
  test.skip(isMobile(page), 'données partagées : modifié une seule fois, sur ordinateur');
  await login(page);
  await page.goto('/budget?mois=2026-12');
  await page.getByRole('button', { name: 'Modifier Restaurants' }).click();
  const dialog = await openedDialog(page);
  await dialog.getByLabel('Montant').fill('250');
  await dialog.getByRole('radio', { name: /décembre 2026 seulement/ }).check();
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  const line = page.getByRole('button', { name: 'Modifier Restaurants' });
  await expect(line).toContainText('250,00');
  await expect(line.locator('mat-icon', { hasText: 'edit_calendar' })).toBeVisible();
  // Le mois suivant garde le montant habituel.
  await page.getByRole('button', { name: 'Mois suivant' }).click();
  await expect(page.getByRole('button', { name: 'Modifier Restaurants' })).toContainText('80,00');
});

test('E18 : ligne sur plusieurs catégories, changées à partir d’un mois (R-BUD-10)', async ({
  page,
}) => {
  test.skip(isMobile(page), 'données partagées : modifié une seule fois, sur ordinateur');
  await login(page);
  await page.goto('/budget?mois=2026-12');
  await page.getByRole('button', { name: 'Modifier Voiture' }).click();
  const dialog = await openedDialog(page);
  const chips = dialog.locator('mat-chip');
  await expect(chips).toHaveText([/Carburant/, /Péage/]);
  await pickCategory(page, dialog, 'Stationnement', 'Stationnement', false);
  await expect(chips).toHaveCount(3);
  await expect(dialog.getByText(/Nouvelles catégories à partir de décembre 2026/)).toBeVisible();
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog).toBeHidden();
  await page.getByRole('button', { name: 'Modifier Voiture' }).click();
  await expect(chips).toHaveCount(3);
  await dialog.getByRole('button', { name: 'Annuler' }).click();
  // Le mois précédent garde ses deux catégories.
  await page.getByRole('button', { name: 'Mois précédent' }).click();
  await page.getByRole('button', { name: 'Modifier Voiture' }).click();
  await expect(chips).toHaveCount(2);
});

test('E20 : créer un projet et y rattacher une transaction (BUD-06)', async ({ page }) => {
  test.skip(isMobile(page), 'données partagées : créé une seule fois, sur ordinateur');
  await login(page);
  await page.goto('/budget/projets');
  await expect(page.getByText('Week-end à Lyon')).toBeVisible();
  await page.getByRole('button', { name: /Nouveau projet/ }).click();
  const dialog = await openedDialog(page);
  await dialog.getByLabel('Nom').fill('Ski 2027');
  await dialog.getByLabel('Montant prévu').fill('1500');
  await dialog.getByRole('button', { name: 'Créer' }).click();
  await expect(page.getByRole('heading', { name: 'Ski 2027' })).toBeVisible();
  await expect(page.getByText(/Aucune transaction/)).toBeVisible();

  await page.goto('/transactions?du=2026-10-16&au=2026-10-16');
  await page.locator('.row mat-checkbox:not(.mat-mdc-checkbox-disabled)').first().click();
  const bulk = page.locator('.bulk');
  await bulk.getByLabel('Projet').click();
  await page.getByRole('option', { name: 'Ski 2027' }).click();
  await bulk.getByRole('button', { name: 'Rattacher' }).click();
  await expect(page.getByText(/1 transaction\(s\) rattachée\(s\)/)).toBeVisible();

  await page.goto('/budget/projets');
  await page.getByRole('link', { name: /Ski 2027/ }).click();
  await expect(page.getByText('Par catégorie')).toBeVisible();
  await expect(page.locator('a.row')).toHaveCount(1);
});

test('E20 : postes d’un projet, hors postes et poste d’une transaction (BUD-11)', async ({
  page,
}) => {
  test.skip(isMobile(page), 'données partagées : modifié une seule fois, sur ordinateur');
  await login(page);
  // La démo détaille son projet en postes (R-BUD-26).
  await page.goto('/budget/projets');
  await page.getByRole('link', { name: /Week-end à Lyon/ }).click();
  await expect(page.getByRole('heading', { name: 'Postes' })).toBeVisible();
  await expect(page.getByText('Hôtel', { exact: true })).toBeVisible();
  await expect(page.getByText('Hors postes', { exact: true })).toHaveCount(0);

  // Le premier poste reprend le montant global ; le champ global disparaît.
  await page.goto('/budget/projets');
  await page.getByRole('link', { name: /Ski 2027/ }).click();
  await page.getByRole('button', { name: /Modifier/ }).click();
  const dialog = await openedDialog(page);
  await dialog.getByRole('button', { name: /Ajouter un poste/ }).click();
  await expect(dialog.getByLabel('Montant prévu')).toHaveCount(0);
  await dialog.getByRole('textbox', { name: 'Poste', exact: true }).fill('Logement');
  await expect(dialog.getByRole('textbox', { name: 'Montant', exact: true })).toHaveValue(
    /1\s?500/,
  );
  await dialog.getByRole('button', { name: /Ajouter un poste/ }).click();
  await dialog.getByRole('textbox', { name: 'Poste', exact: true }).nth(1).fill('Forfaits');
  await dialog.getByRole('textbox', { name: 'Montant', exact: true }).nth(1).fill('400');
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByText('Forfaits', { exact: true })).toBeVisible();
  // La transaction rattachée n'a pas de poste proposé : hors postes (R-BUD-27, R-BUD-28).
  await expect(page.getByText('Hors postes', { exact: true })).toBeVisible();

  // Rangée dans Forfaits depuis son détail.
  await page.locator('a.row').first().click();
  await page.getByLabel('Poste du projet').click();
  await page.getByRole('option', { name: 'Forfaits' }).click();
  await page.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Transaction enregistrée')).toBeVisible();
  await page.goto('/budget/projets');
  await page.getByRole('link', { name: /Ski 2027/ }).click();
  await expect(page.getByText('Forfaits', { exact: true })).toBeVisible();
  await expect(page.getByText('Hors postes', { exact: true })).toHaveCount(0);
});

test('E5 : projet et poste à la saisie rapide, projet en projet écarté (R-BUD-20, R-BUD-27)', async ({
  page,
}) => {
  test.skip(isMobile(page), 'données partagées : créé une seule fois, sur ordinateur');
  await login(page);
  await page.goto('/budget/projets');
  await expect(page.getByRole('heading', { name: 'En projet' })).toBeVisible();
  await expect(page.getByRole('link', { name: /Italie 2027/ })).toBeVisible();

  await openQuickEntry(page);
  const entry = page.locator('app-quick-entry');
  await entry.getByLabel('Montant').fill('30');
  await entry.getByLabel('Compte', { exact: true }).click();
  await page.getByRole('option', { name: 'Compte courant', exact: true }).click();
  await pickCategory(page, entry, 'Restaurants', 'Restaurants');
  await entry.getByLabel('Libellé').fill('Brasserie Lyon');
  await entry.getByLabel('Projet', { exact: true }).click();
  // Seuls les projets en cours sont proposés (R-BUD-24).
  await expect(page.getByRole('option', { name: 'Italie 2027' })).toHaveCount(0);
  await page.getByRole('option', { name: 'Week-end à Lyon' }).click();
  // Le poste suit la catégorie (R-BUD-27).
  await expect(entry.getByLabel('Poste du projet')).toContainText('Restaurants');
  await entry.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Transaction enregistrée')).toBeVisible();

  await page.goto('/budget/projets');
  await page.getByRole('link', { name: /Week-end à Lyon/ }).click();
  await expect(page.getByText(/94,00\s€\s\/\s150,00/)).toBeVisible();
});

test('E21 : centre de notifications (BUD-08)', async ({ page }) => {
  await login(page);
  const bell = page.getByRole('button', { name: /^Notifications/ });
  const alert = page.getByRole('menuitem', { name: /seuil/i }).first();
  // Un clic parti avant que le menu soit prêt ne l'ouvre pas : on réessaie tant qu'il est fermé.
  await expect(async () => {
    if (!(await page.getByRole('menu').isVisible())) await bell.click();
    await expect(alert).toBeVisible({ timeout: 2_000 });
  }).toPass();
  const markAll = page.getByRole('button', { name: 'Tout marquer comme lu' });
  if (await markAll.isVisible()) {
    await markAll.click();
  }
  await expect(markAll).toBeHidden();
});

test('E19 : l’assistant propose des montants d’après les derniers mois (BUD-10)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/budget/assistant');
  await expect(page.getByText(/Moyenne de 6 mois complets/)).toBeVisible();
  await expect(page.getByText('(déjà une ligne)').first()).toBeVisible();
  await expect(page.getByRole('button', { name: 'Créer le budget' })).toBeEnabled();
});

test('accueil : bloc budget du mois (TDB-06)', async ({ page }) => {
  await login(page);
  await expect(page.getByRole('link', { name: 'Budget du mois' })).toBeVisible();
  await expect(page.getByText('Week-end à Lyon').first()).toBeVisible();
});
