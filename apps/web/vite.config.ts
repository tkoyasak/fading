import { cloudflare } from "@cloudflare/vite-plugin";
import { unstable_reactRouterRSC as reactRouterRSC } from "@react-router/dev/vite";
import tailwindcss from "@tailwindcss/vite";
import rsc from "@vitejs/plugin-rsc";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [
    cloudflare({
      viteEnvironment: {
        name: "rsc",
        // Define `ssr` as a child environment so that it runs in the same Worker as the parent `rsc` environment
        childEnvironments: ["ssr"],
      },
    }),
    tailwindcss(),
    reactRouterRSC(),
    rsc({
      // Workaround: Disable the default server handler from @vitejs/plugin-rsc.
      // In preview mode, the plugin tries to import the built RSC entry in Node.js,
      // which fails if your entry uses `cloudflare:*` imports.
      // The Cloudflare plugin handles requests via workerd instead, so this is safe.
      serverHandler: false,
    }),
  ],
  environments: {
    // Workaround: Exclude react-router from dependency optimization in worker environments.
    // The reactRouterRSC plugin adds react-router to optimizeDeps.include at the root level
    // (intended for the client), but this can cause duplicate React instances in the rsc/ssr
    // environments running inside workerd, leading to "Invalid hook call" errors on first load.
    rsc: {
      optimizeDeps: {
        exclude: ["react-router"],
      },
    },
    ssr: {
      optimizeDeps: {
        exclude: ["react-router"],
      },
    },
  },
});
