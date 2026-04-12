import "../global.css";
import {
  createFromReadableStream,
  createFromFetch,
  setServerCallback,
  createTemporaryReferenceSet,
  encodeReply,
} from "@vitejs/plugin-rsc/browser";
import React from "react";
import ReactDOM from "react-dom/client";
import { rscStream } from "rsc-html-stream/client";
import type { RscPayload } from "./entry.rsc.tsx";
import { GlobalErrorBoundary } from "./error-boundary.tsx";
import { createRscRenderRequest } from "./request.tsx";

async function main() {
  // stash `setPayload` function to trigger re-rendering
  // from outside of `BrowserRoot` component (e.g. server function call, navigation, hmr)
  let setPayload: (v: RscPayload) => void;

  // deserialize RSC stream back to React VDOM for CSR
  const initialPayload = await createFromReadableStream<RscPayload>(
    // initial RSC stream is injected in SSR stream as <script>...FLIGHT_DATA...</script>
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- `rsc-html-stream` types `rscStream` as `ReadableStream<any>`
    rscStream as ReadableStream<Uint8Array>,
  );

  // browser root component to (re-)render RSC payload as state
  function BrowserRoot() {
    // oxlint-disable-next-line react/hook-use-state -- `setPayload_` is intentionally wrapped via `startTransition`
    const [payload, setPayload_] = React.useState(initialPayload);

    React.useEffect(() => {
      setPayload = (v) => {
        React.startTransition(() => {
          setPayload_(v);
        });
      };
    }, [setPayload_]);

    // re-fetch/render on client side navigation
    React.useEffect(
      () =>
        listenNavigation(() => {
          void fetchRscPayload();
        }),
      [],
    );

    return payload.root;
  }

  // re-fetch RSC and trigger re-rendering
  async function fetchRscPayload() {
    const renderRequest = createRscRenderRequest(globalThis.location.href);
    const payload = await createFromFetch<RscPayload>(fetch(renderRequest));
    setPayload(payload);
  }

  // register a handler which will be internally called by React
  // on server function request after hydration.
  setServerCallback(async (id, args) => {
    const temporaryReferences = createTemporaryReferenceSet();
    const renderRequest = createRscRenderRequest(globalThis.location.href, {
      id,
      body: await encodeReply(args, { temporaryReferences }),
    });
    const payload = await createFromFetch<RscPayload>(fetch(renderRequest), {
      temporaryReferences,
    });
    setPayload(payload);
    const { ok, data } = payload.returnValue!;
    if (!ok) {
      throw data;
    }
    return data;
  });

  // hydration
  const browserRoot = (
    <React.StrictMode>
      <GlobalErrorBoundary>
        <BrowserRoot />
      </GlobalErrorBoundary>
    </React.StrictMode>
  );
  if ("__NO_HYDRATE" in globalThis) {
    ReactDOM.createRoot(document).render(browserRoot);
  } else {
    ReactDOM.hydrateRoot(document, browserRoot, {
      formState: initialPayload.formState,
    });
  }

  // implement server HMR by triggering re-fetch/render of RSC upon server code change
  if (import.meta.hot) {
    import.meta.hot.on("rsc:update", () => {
      void fetchRscPayload();
    });
  }
}

function onClick(e: MouseEvent) {
  if (!(e.target instanceof Element)) {
    return;
  }
  const link = e.target.closest("a");
  if (
    link instanceof HTMLAnchorElement &&
    link.href &&
    (!link.target || link.target === "_self") &&
    link.origin === location.origin &&
    !link.hasAttribute("download") &&
    e.button === 0 && // left clicks only
    !e.metaKey && // open in new tab (mac)
    !e.ctrlKey && // open in new tab (windows)
    !e.altKey && // download
    !e.shiftKey &&
    !e.defaultPrevented
  ) {
    e.preventDefault();
    history.pushState(null, "", link.href);
  }
}

// a little helper to setup events interception for client side navigation
function listenNavigation(onNavigation: () => void) {
  globalThis.addEventListener("popstate", onNavigation);

  // oxlint-disable-next-line typescript/unbound-method -- saving method reference to patch it; `this` is restored via `apply` in the wrapper
  const oldPushState = globalThis.history.pushState;
  globalThis.history.pushState = function pushState(...args) {
    oldPushState.apply(this, args);
    onNavigation();
  };

  // oxlint-disable-next-line typescript/unbound-method -- saving method reference to patch it; `this` is restored via `apply` in the wrapper
  const oldReplaceState = globalThis.history.replaceState;
  globalThis.history.replaceState = function replaceState(...args) {
    oldReplaceState.apply(this, args);
    onNavigation();
  };

  document.addEventListener("click", onClick);

  return () => {
    document.removeEventListener("click", onClick);
    globalThis.removeEventListener("popstate", onNavigation);
    globalThis.history.pushState = oldPushState;
    globalThis.history.replaceState = oldReplaceState;
  };
}

await main();
