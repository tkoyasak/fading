/*
 * WHAT IS THIS FILE?
 *
 * It's the entry point for Cloudflare Workers when building for production.
 *
 * Learn more about the Cloudflare Workers integration here:
 * - https://qwik.dev/docs/deployments/cloudflare-workers/
 *
 */
 import { createQwikRouter } from '@qwik.dev/router/middleware/cloudflare-pages';
 import render from './entry.ssr';
 
 const fetch = createQwikRouter({ render });
 
export { fetch };
