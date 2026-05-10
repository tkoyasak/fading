import { cloudflarePagesAdapter as cloudflareWorkersAdapter } from "@qwik.dev/router/adapters/cloudflare-pages/vite";
import { mergeConfig } from "vite";
import baseConfig from "../../vite.config";

export default mergeConfig(baseConfig, {
  build: {
    ssr: true,
    rollupOptions: {
      input: ["src/entry.cloudflare-pages.tsx"],
    },
  },
  plugins: [cloudflareWorkersAdapter()],
});
