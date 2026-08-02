import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['*.spec.js'],
    setupFiles: ['./helpers/setup.js'],
    // The console has no build step, so the specs import `web/app.js` directly.
    root: import.meta.dirname,
  },
});
