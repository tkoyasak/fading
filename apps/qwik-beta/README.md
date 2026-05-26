# Qwik App ⚡️

- [Qwik Docs](https://qwik.dev/)
- [Discord](https://qwik.dev/chat)
- [Qwik GitHub](https://github.com/QwikDev/qwik)
- [@QwikDev](https://twitter.com/QwikDev)
- [Vite](https://vitejs.dev/)

---

## Project Structure

This project is using Qwik with [QwikRouter](https://qwik.dev/docs/). QwikRouter is just an extra set of tools on top of Qwik to make it easier to build a full site, including directory-based routing, layouts, and more.

Inside your project, you'll see the following directory structure:

```
├── public/
│   └── ...
└── src/
    ├── components/
    │   └── ...
    └── routes/
        └── ...
```

- `src/routes`: Provides the directory-based routing, which can include a hierarchy of `layout.tsx` layout files, and an `index.tsx` file as the page. Additionally, `index.ts` files are endpoints. Please see the [routing docs](https://qwik.dev/docs/routing/) for more info.

- `src/components`: Recommended directory for components.

- `public`: Any static assets, like images, can be placed in the public directory. Please see the [Vite public directory](https://vitejs.dev/guide/assets.html#the-public-directory) for more info.

## Add Integrations and deployment

Use the `bun qwik add` command to add additional integrations. Some examples of integrations includes: Cloudflare, Netlify or Express Server, and the [Static Site Generator (SSG)](https://qwik.dev/docs/guides/static-site-generation/).

```shell
bun qwik add # or `bun qwik add`
```

## Development

Development mode uses [Vite's development server](https://vitejs.dev/). The `dev` command will server-side render (SSR) the output during development.

```shell
npm start # or `bun start`
```

> Note: during dev mode, Vite may request a significant number of `.js` files. This does not represent a Qwik production build.

## Preview

The preview command will create a production build of the client modules, a production build of `src/entry.preview.tsx`, and run a local server. The preview server is only for convenience to preview a production build locally and should not be used as a production server.

```shell
bun preview # or `bun preview`
```

## Production

The production build will generate client and server modules by running both client and server build commands. The build command will use Typescript to run a type check on the source code.

```shell
bun build # or `bun build`
```

## Cloudflare Workers

Cloudflare's [wrangler](https://github.com/cloudflare/wrangler) CLI can be used to preview a production build locally. To start a local server, run:

```
bun serve
```

Then visit [http://localhost:8787/](http://localhost:8787/)

### Deployments

[Cloudflare Workers](https://workers.cloudflare.com/) can be deployed using the [Wrangler CLI](https://developers.cloudflare.com/workers/wrangler/).

If you don't already have an account, then [create a Cloudflare account here](https://dash.cloudflare.com/sign-up/workers-and-pages).

Once authenticated, you can deploy your Worker:

```
bun deploy
```

### Configuration

The `wrangler.jsonc` file contains your Worker configuration. Key settings include:

- **name**: Your Worker's name
- **main**: Path to your Worker script (default: `./dist/_worker.js`)
- **compatibility_date**: The date used for compatibility with the Workers runtime
- **assets**: Configuration for serving static assets
- **bindings**: Resources your Worker can interact with (KV, R2, D1, etc.)

After adding any binding, use this command to regenerate the worker-configuration.d.ts file

```
bun cf-typegen
```

For more details, see the [Wrangler configuration documentation](https://developers.cloudflare.com/workers/wrangler/configuration/).

### Bindings

Cloudflare Workers can interact with various Cloudflare resources through bindings:

- **KV**: Key-value storage
- **R2**: Object storage
- **D1**: SQL database
- **Durable Objects**: Strongly consistent storage
- **Queues**: Message queues
- **AI**: AI inference

Configure bindings in your `wrangler.jsonc` file. See the [bindings documentation](https://developers.cloudflare.com/workers/runtime-apis/bindings/) for more information.
