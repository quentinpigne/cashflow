import { Page, expect, test } from '@playwright/test';

import { login } from './helpers';

/** Glisse un encadré par sa poignée jusqu'au-dessus d'une cible, pas à pas (le CDK suit le pointeur). */
async function dragTo(
  page: Page,
  widget: string,
  target: ReturnType<Page['locator']>,
): Promise<void> {
  const handle = page.getByRole('button', { name: `Déplacer l’encadré ${widget}` });
  await handle.scrollIntoViewIfNeeded();
  const from = await handle.boundingBox();
  const to = await target.boundingBox();
  if (!from || !to) throw new Error('encadré introuvable');
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  for (let i = 1; i <= 15; i++) {
    await page.mouse.move(
      from.x + ((to.x + 40 - from.x) * i) / 15,
      from.y + ((to.y + 10 - from.y) * i) / 15,
    );
  }
  await page.mouse.up();
}

test('E1 : réorganiser les encadrés de l’accueil, gardé après rechargement', async ({ page }) => {
  test.skip((page.viewportSize()?.width ?? 1280) < 600, 'deux colonnes : ordinateur');
  await login(page);
  const secondColumn = page.locator('.grid > .column').nth(1);
  await dragTo(page, 'Patrimoine', secondColumn.locator('.widget').first());
  await expect(secondColumn.locator('section.net-worth')).toBeVisible();

  await page.reload();
  await expect(page.locator('.grid > .column').nth(1).locator('section.net-worth')).toBeVisible();

  await page.getByRole('button', { name: /Disposition par défaut/ }).click();
  await expect(page.locator('.grid > .column').first().locator('section.net-worth')).toBeVisible();
  await expect(page.getByRole('button', { name: /Disposition par défaut/ })).toBeHidden();
});

test('E1 : dernières saisies, la plus récente en tête (TDB-01)', async ({ page }) => {
  await login(page);
  const widget = page.locator('app-home-recent');
  await expect(widget.locator('a.row')).toHaveCount(5);
  await widget.getByRole('button', { name: 'Réglages de l’encadré Dernières saisies' }).click();
  await page.getByRole('menuitemradio', { name: '10 transactions' }).click();
  await expect(widget.locator('a.row')).toHaveCount(10);
  // Le choix est gardé dans le navigateur.
  await page.reload();
  await expect(page.locator('app-home-recent a.row')).toHaveCount(10);
  await page.locator('app-home-recent a.row').first().click();
  await expect(page).toHaveURL(/\/transactions\/[0-9a-f-]{36}$/);
});

test('E1 : réglages d’un encadré, masquer puis réafficher (TDB-01)', async ({ page }) => {
  await login(page);
  const accounts = page.locator('section.accounts');
  await expect(accounts.locator('a.account').first()).toBeVisible();

  // Option d'affichage : les familles seules, sans le détail des comptes.
  await page.getByRole('button', { name: 'Réglages de l’encadré Comptes' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Détail des comptes' }).click();
  await expect(accounts.locator('a.account')).toHaveCount(0);
  await expect(accounts.locator('.family').first()).toBeVisible();

  // Masquer l'encadré, gardé après rechargement, puis le réafficher.
  await page.getByRole('button', { name: 'Réglages de l’encadré Comptes' }).click();
  await page.getByRole('menuitem', { name: 'Masquer l’encadré' }).click();
  await expect(accounts).toHaveCount(0);
  await page.reload();
  await expect(page.locator('section.accounts')).toHaveCount(0);
  await page.getByRole('button', { name: /Encadrés masqués \(1\)/ }).click();
  await page.getByRole('menuitem', { name: 'Afficher « Comptes »' }).click();
  await expect(accounts.locator('.family').first()).toBeVisible();
  await expect(accounts.locator('a.account')).toHaveCount(0);
});
