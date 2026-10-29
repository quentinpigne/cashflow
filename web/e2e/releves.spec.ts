import { expect, test } from '@playwright/test';

import { isMobile, login, openAccount } from './helpers';

test.describe.configure({ mode: 'serial' });

test('E3 : relevés archivés et mois sans relevé (CPT-11, R-CPT-19, R-CPT-21)', async ({ page }) => {
  await login(page);
  await openAccount(page, 'Compte courant');
  const section = page.locator('app-account-statements');
  await expect(section.getByRole('heading', { name: 'Relevés' })).toBeVisible();
  // Les années sont repliées par défaut.
  const year = section.locator('details', { hasText: '2026' });
  await expect(year).not.toHaveAttribute('open');
  await year.locator('summary').click();
  // Jeu de démonstration : mai, juin, juillet et septembre ; août est sans relevé.
  await expect(section.locator('.row:not(.missing)')).toHaveCount(4);
  await expect(section.locator('.row.missing')).toHaveText(/Août 2026\s*sans relevé/);
  const open = section.getByRole('link', { name: 'Ouvrir le relevé releve-2026-09.pdf' });
  const response = await page.request.get((await open.getAttribute('href'))!);
  expect(response.headers()['content-type']).toBe('application/pdf');
  expect(response.headers()['content-security-policy']).toBe('sandbox');
  if (isMobile(page)) return;

  // Envoyer le relevé d'août : le mois précédent est proposé par défaut.
  const pdf = {
    name: 'releve-aout.pdf',
    mimeType: 'application/pdf',
    buffer: Buffer.from('%PDF-1.4 relevé août e2e'),
  };
  await section.getByRole('button', { name: 'Ajouter' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByLabel('Mois du relevé')).toHaveText('Septembre');
  await dialog.getByLabel('Fichier', { exact: true }).setInputFiles(pdf);
  await dialog.getByLabel('Mois du relevé').click();
  await page.getByRole('option', { name: 'Août' }).click();
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog).toBeHidden();
  await expect(section.locator('.row.missing')).toHaveCount(0);

  // Le même fichier ne s'archive qu'une fois.
  await section.getByRole('button', { name: 'Ajouter' }).click();
  await dialog.getByLabel('Fichier', { exact: true }).setInputFiles(pdf);
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog.getByRole('alert')).toContainText('déjà archivé (Relevé Compte courant');
  await dialog.getByRole('button', { name: 'Annuler' }).click();

  await section.getByRole('button', { name: 'Actions sur le relevé releve-aout.pdf' }).click();
  await page.getByRole('menuitem', { name: 'Supprimer' }).click();
  await page.getByRole('button', { name: 'Supprimer' }).click();
  await expect(section.locator('.row.missing')).toHaveText(/Août 2026\s*sans relevé/);
});
