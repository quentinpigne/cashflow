import { Page, expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

test.describe.configure({ mode: 'serial' });

/** Déplie les conditions du crédit (repliées sur mobile). */
async function openConditions(page: Page): Promise<void> {
  const header = page.getByRole('button', { name: /Conditions du crédit/ });
  if ((await header.getAttribute('aria-expanded')) === 'false') await header.click();
}

test('E36 : situation, projet, trois questions et demande préparée (CRE-07, CRE-08, CRE-11)', async ({
  page,
}) => {
  test.setTimeout(120_000);
  await login(page);
  await page.goto('/prets/simuler');
  await expect(page.getByRole('tab', { name: 'Simuler' })).toHaveAttribute('aria-selected', 'true');
  const shot = (name: string) =>
    page.screenshot({
      path: `test-results/credit-${name}-${isMobile(page) ? 'mobile' : 'ordinateur'}.png`,
      fullPage: true,
    });

  // Que puis-je acheter ? Une réserve plus grande diminue le prix maximal ; la situation est gardée (R-CRE-30).
  await page.getByRole('option', { name: 'Ancien' }).click();
  await page.getByRole('radio', { name: /Que puis-je acheter|Acheter/ }).click();
  const result = page.locator('section.result');
  await expect(result).toContainText("Prix d'achat maximal");
  const price = result.locator('.headline');
  await expect(price).toHaveText(/€/);
  const before = (await price.textContent()) ?? '';
  const situation = page.locator('app-situation-bar');
  await situation.getByRole('button', { name: 'Ajuster' }).click();
  const panel = page.getByRole('dialog');
  await panel.getByLabel('Réserve de sécurité').fill('12');
  await expect(price).not.toHaveText(before);
  await page.reload();
  await situation.getByRole('button', { name: 'Ajuster' }).click();
  await expect(panel.getByLabel('Réserve de sécurité')).toHaveValue('12');
  await panel.getByLabel('Réserve de sécurité').fill('6');
  await expect(price).toHaveText(before);
  await panel.getByRole('button', { name: 'Fermer' }).click();
  await expect(panel).toBeHidden();
  await openConditions(page);
  await shot('acheter');

  // Puis-je financer cet achat ? Une voiture à 25 000 € avec 5 000 € d'apport, à 5 % sur 60 mois (R-CRE-40).
  await page.getByRole('option', { name: 'Véhicule' }).click();
  await page.getByRole('radio', { name: /Puis-je financer|Financer/ }).click();
  await page.getByLabel('Prix', { exact: true }).fill('25 000');
  await page.getByLabel('Apport', { exact: true }).fill('5 000');
  await expect(page.getByLabel('Frais de notaire')).toHaveCount(0);
  await expect(page.locator('mat-panel-description')).toContainText('sans assurance');
  await expect(result).toContainText('20 000,00');
  await expect(result).toContainText('377,42');
  await expect(result).toContainText("Taux d'endettement");
  await shot('financer');

  // Que coûte ce prêt ? 200 000 € sur 20 ans à 3 %, assurance 0,3 % (R-CRE-41).
  await page.getByRole('option', { name: 'Ancien' }).click();
  await page.getByRole('radio', { name: /Que coûte ce prêt|Coût/ }).click();
  await openConditions(page);
  await page.getByLabel('Taux nominal').fill('3');
  await page.getByLabel('Durée', { exact: true }).fill('240');
  await page.getByRole('radio', { name: 'La mensualité' }).click();
  await page.getByLabel('Capital', { exact: true }).fill('200 000');
  await expect(result).toContainText('1 159,20');
  await page.getByRole('radio', { name: 'Le capital' }).click();
  await page.getByLabel('Mensualité visée').fill('1 200');
  await expect(result).toContainText('Capital empruntable');
  await shot('cout');
  await page.getByRole('button', { name: 'Conditions par défaut' }).click();
  await expect(page.getByLabel('Taux nominal')).toHaveValue('3,20');
  if (isMobile(page)) return;

  // Une simulation ouverte passe dans « Que coûte ce prêt ? », avec ses conditions (R-CRE-37).
  await page.locator('.row', { hasText: '25 ans à 3,4 %' }).getByRole('button').first().click();
  await expect(page.getByLabel('Taux nominal')).toHaveValue('3,40');
  await expect(page.getByRole('heading', { name: '25 ans à 3,4 %' })).toBeVisible();
  await page.getByRole('button', { name: 'Conditions par défaut' }).click();

  // Enregistrer, puis préparer une demande depuis la simulation (R-CRE-36).
  await page.getByRole('radio', { name: 'Le capital' }).click();
  await result.getByRole('button', { name: 'Enregistrer' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Nom', { exact: true }).fill('Capacité 1 200 €');
  await dialog.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(dialog).toBeHidden();
  const row = page.locator('.row', { hasText: 'Capacité 1 200 €' });
  await row.getByRole('button', { name: 'Préparer une demande' }).click();
  await expect(page.getByRole('heading', { name: 'Capacité 1 200 €', level: 1 })).toBeVisible();
  await expect(page.locator('section.capacity')).toContainText('Capacité 1 200 €');

  // Une demande préparée depuis « Puis-je financer cet achat ? » reprend le prix et l'apport.
  await page.goto('/prets/simuler');
  await page.getByRole('option', { name: 'Véhicule' }).click();
  await page.getByRole('radio', { name: /Puis-je financer|Financer/ }).click();
  await expect(page.getByLabel('Prix', { exact: true })).toHaveValue(/^25\s?000,00$/);
  await expect(result).toContainText('377,42');
  await result.getByRole('button', { name: 'Préparer une demande' }).click();
  await expect(
    page.getByRole('heading', { name: /Véhicule à 25\s000,00/, level: 1 }),
  ).toBeVisible();
  await expect(page.locator('.totals')).toContainText('20 000,00');
});

test("E42 : conditions par défaut d'un type de projet (CRE-12, R-CRE-38)", async ({ page }) => {
  test.skip(isMobile(page), 'parcours vérifié sur ordinateur');
  await login(page);
  await page.goto('/reglages');
  await page.getByRole('link', { name: 'Conditions de crédit' }).click();
  const vehicle = page.getByRole('region', { name: 'Véhicule' });
  await vehicle.getByLabel('Taux nominal').fill('4,5');
  await vehicle.getByLabel('Durée', { exact: true }).fill('48');
  await expect(vehicle.getByRole('button', { name: 'Valeurs initiales' })).toBeEnabled();
  await page.screenshot({ path: 'test-results/credit-conditions-par-defaut.png', fullPage: true });

  // Le projet « Véhicule » charge ses conditions par défaut (R-CRE-39).
  await page.waitForTimeout(800);
  await page.goto('/prets/simuler');
  await page.getByRole('option', { name: 'Ancien' }).click();
  await page.getByRole('option', { name: 'Véhicule' }).click();
  await openConditions(page);
  await expect(page.getByLabel('Taux nominal')).toHaveValue('4,50');
  await expect(page.getByLabel('Durée', { exact: true })).toHaveValue('48');

  // Valeurs initiales rétablies.
  await page.goto('/reglages/conditions-credit');
  await vehicle.getByRole('button', { name: 'Valeurs initiales' }).click();
  await expect(vehicle.getByLabel('Taux nominal')).toHaveValue('5,00');
  await page.waitForTimeout(800);
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
