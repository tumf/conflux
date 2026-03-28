import { defineConfig, loadEnv } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, __dirname, 'CFLX_');
  const proxyTarget = env.CFLX_API_PROXY_TARGET;

  return {
    plugins: [react()],
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src'),
      },
    },
    build: {
      outDir: 'dist',
      sourcemap: false,
    },
    base: proxyTarget ? '/' : '/dashboard/',
    server: {
      allowedHosts: true,
      ...(proxyTarget && {
        proxy: {
          '/api': {
            target: proxyTarget,
            changeOrigin: true,
            secure: true,
            ws: true,
          },
        },
      }),
    },
  };
})
