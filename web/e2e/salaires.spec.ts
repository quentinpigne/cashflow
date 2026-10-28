import { expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

test.describe.configure({ mode: 'serial' });

/** Ouvre la rubrique Salaires : menu latéral sur ordinateur, ⋮ sur mobile. */
async function openSalaries(page: import('@playwright/test').Page) {
  if (isMobile(page)) {
    await page.getByRole('button', { name: 'Menu' }).click();
    await page.getByRole('menuitem', { name: 'Salaires' }).click();
  } else {
    await page.locator('mat-sidenav').getByRole('link', { name: 'Salaires', exact: true }).click();
  }
  await expect(page.getByRole('heading', { name: 'Salaires', level: 1 })).toBeVisible();
}

test('E29, E30 : évolution, rémunérations et reçu d’un emploi (SAL-01 à SAL-03)', async ({
  page,
}) => {
  await login(page);
  await openSalaries(page);
  await expect(page.getByRole('heading', { name: 'Évolution' })).toBeVisible();
  await expect(page.locator('[echarts] canvas').first()).toBeVisible();
  await page.getByRole('link', { name: /Acme Industries/ }).click();
  await expect(page.getByRole('heading', { name: 'Acme Industries' })).toBeVisible();
  await expect(page.locator('.balance')).toContainText('3 400,00 €');
  await expect(page.getByRole('button', { name: /Actions sur la rémunération/ })).toHaveCount(3);
  if (!isMobile(page)) {
    await expect(page.getByRole('cell', { name: 'Promotion' })).toBeVisible();
  }
  await expect(page.getByRole('button', { name: 'Actions sur la prime 13e mois' })).toBeVisible();
  // Le salaire de septembre a été reçu : aucun écart signalé.
  await expect(page.getByRole('heading', { name: 'Reçu' })).toBeVisible();
  await expect(page.locator('tr.flagged')).toHaveCount(0);
});

test('E30 : une augmentation prévue, préremplie avec la dernière rémunération (R-SAL-2, R-SAL-4)', async ({
  page,
}) => {
  test.skip(isMobile(page), 'données partagées : créée une seule fois, sur ordinateur');
  await login(page);
  await openSalaries(page);
  await page.getByRole('link', { name: /Acme Industries/ }).click();
  await page.getByRole('button', { name: 'Ajouter' }).first().click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByLabel('Net mensuel versé')).toHaveValue('3400,00');
  await dialog.getByLabel('Année').fill('2027');
  await dialog.getByLabel("Mois d'effet").click();
  await page.getByRole('option', { name: 'Mars' }).click();
  await dialog.getByLabel('Net mensuel versé').fill('3600');
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('prévue')).toBeVisible();

  // Elle apparaît dans les hypothèses de la projection du salaire, puis on la retire.
  await page.goto('/projections');
  await page.getByRole('tab', { name: 'Salaire' }).click();
  await expect(page.getByText(/Prévu : Rémunération chez Acme Industries/)).toBeVisible();
  await page.goBack();
  await page.getByRole('button', { name: /Actions sur la rémunération de 2027-03-01/ }).click();
  await page.getByRole('menuitem', { name: 'Supprimer' }).click();
  await expect(page.getByText('prévue')).toHaveCount(0);
});

test('E24 : salaire projeté et capacité d’épargne (SAL-04, PRJ-10)', async ({ page }) => {
  await login(page);
  await page.goto('/projections');
  await expect(page.getByRole('heading', { name: "Capacité d'épargne" })).toBeVisible();
  await expect(page.getByText(/Crédité sur Compte courant/)).toBeVisible();
  await page.getByRole('tab', { name: 'Salaire' }).click();
  await expect(page.getByText('Médian en 2036')).toBeVisible();
  await expect(page.getByText(/net annuel, \+2,0\s%/)).toBeVisible();
});

test('E9 : rubrique Salaires et tolérance de l’écart (R-SAL-14, R-SAL-9)', async ({ page }) => {
  await login(page);
  await page.goto('/reglages');
  await expect(page.getByRole('switch', { name: 'Salaires' })).toBeDisabled();
  await expect(page.getByLabel("Tolérance de l'écart de salaire")).toHaveValue('5,00');
});

test('E30 : fiches de paie archivées et mois sans fiche (SAL-09, R-SAL-16, R-SAL-18)', async ({
  page,
}) => {
  await login(page);
  await openSalaries(page);
  await page.getByRole('link', { name: /Acme Industries/ }).click();
  const section = page.locator('section.payslips');
  await expect(section.getByRole('heading', { name: 'Fiches de paie' })).toBeVisible();
  // Les années sont repliées par défaut.
  const year = section.locator('details', { hasText: '2026' });
  await expect(year).not.toHaveAttribute('open');
  await year.locator('summary').click();
  // Jeu de démonstration : juin, juillet et septembre ; août est sans fiche.
  await expect(section.locator('.row.payslip:not(.missing)')).toHaveCount(3);
  await expect(section.locator('.row.missing')).toHaveText(/Août 2026\s*sans fiche/);
  const open = section.getByRole('link', { name: 'Ouvrir la fiche fiche-de-paie-2026-09.pdf' });
  const response = await page.request.get((await open.getAttribute('href'))!);
  expect(response.headers()['content-type']).toBe('application/pdf');
  expect(response.headers()['content-security-policy']).toBe('sandbox');
  if (isMobile(page)) return;

  // Envoyer la fiche d'août : le mois précédent est proposé par défaut.
  const pdf = {
    name: 'paie-aout.pdf',
    mimeType: 'application/pdf',
    buffer: Buffer.from('%PDF-1.4 août e2e'),
  };
  await section.getByRole('button', { name: 'Ajouter' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByLabel('Mois de paie')).toHaveText('Septembre');
  await dialog.getByLabel('Fichier de la fiche de paie').setInputFiles(pdf);
  await dialog.getByLabel('Mois de paie').click();
  await page.getByRole('option', { name: 'Août' }).click();
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog).toBeHidden();
  await expect(section.locator('.row.missing')).toHaveCount(0);

  // Le même fichier ne s'archive qu'une fois.
  await section.getByRole('button', { name: 'Ajouter' }).click();
  await dialog.getByLabel('Fichier de la fiche de paie').setInputFiles(pdf);
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog.getByRole('alert')).toContainText('déjà archivé (fiche de 08/2026)');
  await dialog.getByRole('button', { name: 'Annuler' }).click();

  await section.getByRole('button', { name: 'Actions sur la fiche paie-aout.pdf' }).click();
  await page.getByRole('menuitem', { name: 'Supprimer' }).click();
  await page.getByRole('button', { name: 'Supprimer' }).click();
  await expect(section.locator('.row.missing')).toHaveText(/Août 2026\s*sans fiche/);
});
