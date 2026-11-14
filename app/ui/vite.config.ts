import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { readFileSync } from 'node:fs';

const { version } = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')) as { version: string };

// `npm run dev` sends /api to the test server (`prism serve`, port 4317), so the screens run against the real engine
// in a browser. A Tauri build ignores the proxy and talks to the engine directly.
export default defineConfig({
  base: './',
  define: { __APP_VERSION__: JSON.stringify(version) },
  plugins: [react()],
  build: { outDir: 'dist', sourcemap: false },
  server: {
    port: 5317,
    proxy: { '/api': { target: process.env.PRISM_SERVER ?? 'http://127.0.0.1:4317', changeOrigin: false } },
  },
});
