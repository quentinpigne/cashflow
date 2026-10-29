import { expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

test.describe.configure({ mode: 'serial' });

test('E36 : simulateur, calcul inverse et enregistrement (CRE-01, CRE-02)', async ({ page }) => {
  await login(page);
  await page.goto('/prets/simulations');
  await expect(page.getByRole('tab', { name: 'Simulations' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  const simulator = page.locator('section.simulator');
  // Valeurs de départ : 200 000 € sur 240 mois à 3,2 %, assurance 0,3 %.
  await simulator.getByLabel('Taux nominal').fill('3');
  await expect(simulator.locator('.figure').first()).toContainText('1 159,20');
  await expect(simulator).toContainText('TAEG');

  // Le capital empruntable pour 1 200 € par mois.
  await simulator.getByRole('radio', { name: 'Le capital' }).click();
  await simulator.getByLabel('Mensualité visée').fill('1 200');
  await expect(simulator.locator('.figure.main')).toContainText('Capital empruntable');
  await page.screenshot({
    path: `test-results/credit-simulateur-${isMobile(page) ? 'mobile' : 'ordinateur'}.png`,
    fullPage: true,
  });
  if (isMobile(page)) return;

  await simulator.getByRole('button', { name: 'Enregistrer' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Nom', { exact: true }).fill('Capacité 1 200 €');
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('.row', { hasText: 'Capacité 1 200 €' })).toBeVisible();
});

test('E39 : demande, capacité, pièces, démarche et offre acceptée (CRE-04 à CRE-06)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/prets/demandes');
  await page.getByRole('link', { name: /Maison Annecy/ }).click();
  await expect(page.getByRole('heading', { name: 'Maison Annecy', level: 1 })).toBeVisible();
  // Jeu de démonstration : prix 420 000 €, notaire estimé 31 500 €, travaux 20 000 €, apport 60 000 €.
  await expect(page.locator('.totals')).toContainText('411 500,00');
  const capacity = page.locator('section.capacity');
  await expect(capacity).toContainText("Taux d'endettement");
  await expect(capacity).toContainText('Camille');
  await expect(page.locator('.piece', { hasText: 'fiches de paie' }).first()).toBeVisible();
  await page.screenshot({
    path: `test-results/credit-demande-${isMobile(page) ? 'mobile' : 'ordinateur'}.png`,
    fullPage: true,
  });
  if (isMobile(page)) return;

  // Une offre du courtier, acceptée, transformée en prêt.
  const requests = page.locator('section.card', {
    has: page.getByRole('heading', { name: 'Démarches' }),
  });
  const request = requests.locator('.row', { hasText: 'Courtier Alpes Crédit' });
  await request.getByRole('button', { name: /Actions sur/ }).click();
  await page.getByRole('menuitem', { name: "Enregistrer l'offre" }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Taux nominal').fill('3,35');
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog).toBeHidden();
  await expect(request).toContainText('Offre reçue');

  await request.getByRole('button', { name: /Actions sur/ }).click();
  await page.getByRole('menuitem', { name: "Accepter l'offre" }).click();
  await page.getByRole('button', { name: 'Accepter' }).click();
  await page.getByRole('button', { name: 'Créer le prêt' }).click();
  const loan = page.getByRole('dialog');
  await expect(loan.getByLabel('Nom', { exact: true })).toHaveValue('Offre Courtier Alpes Crédit');
  await loan.getByLabel('Première échéance').fill('05/12/2026');
  await loan.getByRole('button', { name: 'Créer le prêt' }).click();
  await expect(
    page.getByRole('heading', { name: 'Offre Courtier Alpes Crédit', level: 1 }),
  ).toBeVisible();
});

test("E40 : capacité d'achat, années à venir et demande créée (CRE-07)", async ({ page }) => {
  await login(page);
  await page.goto('/prets/capacite');
  await expect(page.getByRole('tab', { name: 'Capacité' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  const today = page.locator('section.today');
  await expect(today).toContainText("Prix d'achat maximal aujourd'hui");
  await expect(page.getByRole('heading', { name: 'Dans les années à venir' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Sensibilité' })).toBeVisible();
  const before = await today.locator('.price').textContent();

  // Une réserve plus grande diminue l'apport, donc le prix maximal ; le choix est gardé.
  await page.getByLabel('Réserve de sécurité').fill('12');
  await expect(today.locator('.price')).not.toHaveText(before ?? '');
  await page.screenshot({
    path: `test-results/credit-capacite-${isMobile(page) ? 'mobile' : 'ordinateur'}.png`,
    fullPage: true,
  });
  await page.reload();
  await expect(page.getByLabel('Réserve de sécurité')).toHaveValue('12');
  await page.getByLabel('Réserve de sécurité').fill('6');
  await expect(today.locator('.price')).toHaveText(before ?? '');
  if (isMobile(page)) return;

  await today.getByRole('button', { name: 'Créer une demande' }).click();
  await expect(page.getByRole('heading', { name: /^Achat jusqu'à/, level: 1 })).toBeVisible();
  await expect(page.locator('section.capacity')).toContainText('Capacité d');
});
