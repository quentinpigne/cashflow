import { defineConfig, devices } from '@playwright/test';

/**
 * Tests de bout en bout (architecture.md § Stratégie de test, ADR-0018) : l'interface compilée (`ng build`),
 * servie par le vrai binaire, sur le jeu de démonstration à date figée.
 */
export default defineConfig({
  testDir: './e2e',
  workers: 1,
  // Un écran qui calcule beaucoup (accueil, analyses) peut dépasser 5 s sur une machine chargée ou en CI.
  expect: { timeout: 10_000 },
  reporter: [['list']],
  use: {
    baseURL: 'http://127.0.0.1:3100',
    locale: 'fr-FR',
    timezoneId: 'Europe/Paris',
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'ordinateur',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 860 } },
    },
    { name: 'mobile', use: { ...devices['Pixel 7'] } },
  ],
  webServer: {
    command: 'sh e2e/serve.sh',
    url: 'http://127.0.0.1:3100/api/v1/health',
    timeout: 600_000,
    reuseExistingServer: false,
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
