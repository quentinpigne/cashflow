import { expect, test } from '@playwright/test';

import { isMobile, login, openedDialog } from './helpers';

test.describe.configure({ mode: 'serial' });

test('E34 : recherche dans la bibliothèque et documents à classer (DOC-01, DOC-03)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/documents');
  await expect(page.getByRole('heading', { name: 'Documents', level: 1 })).toBeVisible();
  // Jeu de démonstration : un courrier envoyé sans type.
  await expect(page.getByRole('button', { name: 'À classer (1)' })).toBeVisible();

  // Par le titre, puis par le contenu du PDF, l'extrait montrant le mot trouvé (R-DOC-18, R-DOC-19).
  const search = page.getByLabel(/Rechercher dans les titres/);
  await search.fill('offre pret');
  await expect(page.locator('.row')).toHaveCount(1);
  await expect(page.locator('.row')).toContainText('offre-de-pret-immobilier');
  await search.fill('MULTIRISQUE');
  // La liste précédente compte aussi une ligne : attendre la nouvelle.
  await expect(page.locator('.row').first()).toContainText('Assurance habitation');
  await expect(page.locator('.row')).toHaveCount(1);
  await expect(page.locator('.row .snippet strong')).toHaveText('Multirisque');
  // Par le nom d'un objet rattaché.
  await search.fill('acme');
  await expect(page.locator('.row').first()).toContainText('Fiche de paie Acme Industries');
  await search.fill('');

  await page.getByRole('button', { name: 'À classer (1)' }).click();
  await expect(page.locator('.row')).toHaveCount(1);
  await expect(page.locator('.row')).toContainText('scan-courrier');
});

test('E34, E35 : envoi, classement et rattachement (R-DOC-3, R-DOC-9, R-DOC-14)', async ({
  page,
}) => {
  test.skip(isMobile(page), 'parcours sur ordinateur');
  await login(page);
  await page.goto('/documents');
  const contract = {
    name: 'Contrat box internet.pdf',
    mimeType: 'application/pdf',
    buffer: Buffer.from('%PDF-1.4 contrat box e2e'),
  };
  const word = {
    name: 'lettre.pdf',
    mimeType: 'application/pdf',
    buffer: Buffer.from('PK\u0003\u0004word/document.xml'),
  };
  await page.getByLabel('Fichiers à envoyer').setInputFiles([contract, word]);
  const refusals = await openedDialog(page);
  await expect(refusals.getByRole('heading')).toHaveText('Un fichier refusé');
  await expect(refusals).toContainText('lettre.pdf');
  await refusals.getByRole('button', { name: 'Fermer' }).click();
  await expect(refusals).toHaveCount(0);

  // Le même fichier est refusé, avec le document existant (R-DOC-3).
  await page.getByLabel('Fichiers à envoyer').setInputFiles([contract]);
  await expect(refusals).toContainText('déjà archivé (Contrat box internet)');
  await refusals.getByRole('button', { name: 'Ouvrir le document existant' }).click();

  // Fiche du document : classé en contrat, rattaché à un compte.
  await expect(page.getByRole('heading', { name: 'Contrat box internet', level: 1 })).toBeVisible();
  await page.getByLabel('Type').click();
  await page.getByRole('option', { name: 'Contrat', exact: true }).click();
  await page.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Document enregistré')).toBeVisible();

  await page.getByRole('button', { name: 'Rattacher à…' }).click();
  const dialog = await openedDialog(page);
  await dialog.getByLabel('Compte', { exact: true }).fill('Compte courant');
  await page
    .getByRole('option', { name: /^Compte courant/ })
    .first()
    .click();
  await dialog.getByRole('button', { name: 'Rattacher' }).click();
  const chip = page.locator('.chip', { hasText: 'Compte courant' });
  await expect(chip).toBeVisible();

  // Il apparaît dans la section Documents du compte.
  await chip.getByRole('link').click();
  const section = page.locator('app-document-section');
  await expect(section.getByRole('link', { name: /^Contrat box internet Contrat/ })).toBeVisible();
});

test('E7 : justificatif d’une transaction (DOC-04)', async ({ page }) => {
  test.skip(isMobile(page), 'parcours sur ordinateur');
  await login(page);
  await page.goto('/documents');
  await page.getByLabel(/Rechercher dans les titres/).fill('decathlon');
  await expect(page.locator('.row')).toHaveCount(1);
  await page.locator('.row a.main').click();
  await page.locator('.chip a', { hasText: 'CB DECATHLON' }).click();
  const justificatifs = page.locator('app-document-section');
  await expect(justificatifs.getByRole('heading', { name: 'Justificatifs' })).toBeVisible();
  await expect(justificatifs.locator('.tile')).toHaveCount(1);
  await expect(justificatifs.locator('.tile')).toContainText('ticket-decathlon');
});
