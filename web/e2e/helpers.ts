import { Page, expect } from '@playwright/test';

export const PASSWORD = 'demo-cashflow';
/** Date du jour du serveur de test (e2e/serve.sh) : le navigateur vit à la même date. */
export const TODAY = '2026-10-20T10:00:00';

export async function login(page: Page): Promise<void> {
  await page.clock.install({ time: new Date(TODAY) });
  await page.goto('/connexion');
  await page.getByLabel('Mot de passe').fill(PASSWORD);
  await page.getByRole('button', { name: 'Se connecter' }).click();
  await expect(page.locator('section.net-worth')).toBeVisible();
}

export function isMobile(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1280) < 600;
}

/** Choisit une catégorie dans un sélecteur à autocomplétion. */
export async function pickCategory(
  page: Page,
  scope: ReturnType<Page['locator']>,
  search: string,
  option: string,
  /** Vérifier le texte affiché ; inutile quand le choix fait disparaître le champ (accueil). */
  checkDisplay = true,
): Promise<void> {
  const input = scope.locator('app-category-picker input').first();
  await input.focus();
  await input.fill(search);
  await page.getByRole('option', { name: option, exact: true }).first().click();
  // Le champ affiche la catégorie choisie (« Parent › Enfant »), jamais « — ».
  if (checkDisplay) {
    await expect(input).toHaveValue(new RegExp(`${option}$`));
  }
}

/** Ouvre la saisie rapide depuis le bouton « + » ou « Nouvelle transaction ». */
export async function openQuickEntry(page: Page): Promise<void> {
  await page.getByRole('button', { name: /Nouvelle transaction/ }).click();
  await expect(page.getByRole('button', { name: 'Enregistrer' })).toBeVisible();
}

/** Ouvre la fiche d'un compte depuis la liste des comptes. */
export async function openAccount(page: Page, name: string): Promise<void> {
  await page.goto('/comptes');
  await page
    .getByRole('link', { name: new RegExp(`^${name}`) })
    .first()
    .click();
  await expect(page.getByRole('heading', { name })).toBeVisible();
}

/** Une ligne de transaction de la fiche compte (tableau sur ordinateur, liste sur mobile). */
export function transactionRow(page: Page, label: string) {
  return isMobile(page)
    ? page.locator('button.item', { hasText: label })
    : page.getByRole('row', { name: new RegExp(label) });
}
