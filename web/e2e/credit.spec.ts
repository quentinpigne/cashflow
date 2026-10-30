import { expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

test.describe.configure({ mode: 'serial' });

test('E36 : capacité, simulateur et demande préparée (CRE-01, CRE-02, CRE-07, CRE-08)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/prets/simuler');
  await expect(page.getByRole('tab', { name: 'Simuler' })).toHaveAttribute('aria-selected', 'true');

  // Capacité d'achat : une réserve plus grande diminue le prix maximal ; le choix est gardé.
  const today = page.locator('section.today');
  await expect(today).toContainText("Prix d'achat maximal aujourd'hui");
  await expect(page.getByRole('heading', { name: 'Sensibilité' })).toBeVisible();
  const before = await today.locator('.price').textContent();
  await page.getByLabel('Réserve de sécurité').fill('12');
  await expect(today.locator('.price')).not.toHaveText(before ?? '');
  await page.reload();
  await expect(page.getByLabel('Réserve de sécurité')).toHaveValue('12');
  await page.getByLabel('Réserve de sécurité').fill('6');
  await expect(today.locator('.price')).toHaveText(before ?? '');
  await page.screenshot({
    path: `test-results/credit-simuler-${isMobile(page) ? 'mobile' : 'ordinateur'}.png`,
    fullPage: true,
  });

  // Le simulateur part de la capacité ; 200 000 € sur 20 ans à 3 %, assurance 0,3 %.
  const simulator = page.locator('section.simulator');
  await simulator.getByLabel('Capital').fill('200 000');
  await simulator.getByLabel('Durée').fill('240');
  await simulator.getByLabel('Taux nominal').fill('3');
  await expect(simulator.locator('.figure').first()).toContainText('1 159,20');
  await simulator.getByRole('radio', { name: 'Le capital' }).click();
  await simulator.getByLabel('Mensualité visée').fill('1 200');
  await expect(simulator.locator('.figure.main')).toContainText('Capital empruntable');
  await simulator.screenshot({
    path: `test-results/credit-simulateur-${isMobile(page) ? 'mobile' : 'ordinateur'}.png`,
  });
  if (isMobile(page)) return;

  await simulator.getByRole('button', { name: 'Enregistrer' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Nom', { exact: true }).fill('Capacité 1 200 €');
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog).toBeHidden();
  const row = page.locator('.row', { hasText: 'Capacité 1 200 €' });
  await row.getByRole('button', { name: 'Préparer une demande' }).click();
  await expect(page.getByRole('heading', { name: 'Capacité 1 200 €', level: 1 })).toBeVisible();
  await expect(page.locator('section.capacity')).toContainText('Capacité 1 200 €');
});

test('E39 : relance, offre et acceptation en bien et prêt (CRE-04 à CRE-06, CRE-09, CRE-10)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/prets/demandes');
  // Le parcours sur ordinateur relance la démarche : l'état « à relancer » n'est vérifié qu'une fois.
  const fresh = !isMobile(page);
  if (fresh) {
    await expect(page.locator('a.row', { hasText: 'Maison Annecy' })).toContainText(
      '1 démarche à relancer',
    );
  }
  await page.getByRole('link', { name: /Maison Annecy/ }).click();
  await expect(page.getByRole('heading', { name: 'Maison Annecy', level: 1 })).toBeVisible();
  await expect(page.locator('.totals')).toContainText('411 500,00');
  await expect(page.locator('section.capacity')).toContainText("Taux d'endettement");
  await expect(page.locator('.piece', { hasText: 'fiches de paie' }).first()).toBeVisible();

  // La démarche envoyée il y a trois semaines est à relancer (R-CRE-26).
  const requests = page.locator('section.card', {
    has: page.getByRole('heading', { name: 'Démarches' }),
  });
  const request = requests.locator('.request', { hasText: 'Courtier Alpes Crédit' });
  if (fresh) await expect(request.locator('.tag.due')).toHaveText('à relancer');
  await page.screenshot({
    path: `test-results/credit-demande-${isMobile(page) ? 'mobile' : 'ordinateur'}.png`,
    fullPage: true,
  });
  if (isMobile(page)) return;

  await request.getByRole('button', { name: 'Relancer', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Comment').fill('Mail au courtier');
  await dialog.getByRole('button', { name: 'Relancer' }).click();
  await expect(dialog).toBeHidden();
  await expect(request.locator('.tag.due')).toHaveCount(0);
  await expect(request.locator('.journal')).toContainText('Mail au courtier');
  await requests.screenshot({ path: 'test-results/credit-demarches.png' });

  // Une offre, acceptée : l'assistant crée le bien puis le prêt (R-CRE-27).
  await request
    .getByRole('button', { name: 'Actions sur Courtier Alpes Crédit', exact: true })
    .click();
  await page.getByRole('menuitem', { name: "Enregistrer l'offre" }).click();
  await dialog.getByLabel('Taux nominal').fill('3,35');
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog).toBeHidden();
  await expect(request).toContainText('Offre reçue');

  await request
    .getByRole('button', { name: 'Actions sur Courtier Alpes Crédit', exact: true })
    .click();
  await page.getByRole('menuitem', { name: "Accepter l'offre" }).click();
  await page.getByRole('button', { name: 'Accepter' }).click();
  const assistant = page.getByRole('dialog', { name: /Offre acceptée/ });
  await assistant.getByLabel("Date d'achat").fill('15/01/2027');
  await assistant.getByRole('button', { name: 'Créer le bien' }).click();
  await expect(assistant).toContainText('Bien créé');
  await assistant.getByRole('button', { name: 'Suivant' }).first().click();
  await assistant.getByRole('button', { name: 'Créer le prêt' }).click();
  const loan = page.getByRole('dialog').last();
  await expect(loan.getByLabel('Nom', { exact: true })).toHaveValue('Offre Courtier Alpes Crédit');
  await loan.getByLabel('Première échéance').fill('05/02/2027');
  await loan.getByRole('button', { name: 'Créer le prêt' }).click();
  await expect(assistant).toContainText('Prêt créé');
  await assistant.getByRole('button', { name: 'Suivant' }).last().click();
  await assistant.getByRole('button', { name: 'Terminer' }).click();
  await expect(
    page.getByRole('heading', { name: 'Offre Courtier Alpes Crédit', level: 1 }),
  ).toBeVisible();
});
