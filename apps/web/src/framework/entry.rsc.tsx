import {
  renderToReadableStream,
  createTemporaryReferenceSet,
  decodeReply,
  loadServerAction,
  decodeAction,
  decodeFormState,
} from "@vitejs/plugin-rsc/rsc";
import type { ReactFormState } from "react-dom/client";
import { Root } from "../root.tsx";
import { parseRenderRequest } from "./request.tsx";

export interface RscPayload {
  root: React.ReactNode;
  returnValue?: { ok: boolean; data: unknown };
  formState?: ReactFormState;
}

async function handler(request: Request): Promise<Response> {
  // differentiate RSC, SSR, action, etc.
  const renderRequest = parseRenderRequest(request);
  // oxlint-disable-next-line prefer-destructuring -- reassigning a parameter, destructuring would conflict
  request = renderRequest.request;

  // handle server function request
  let returnValue: RscPayload["returnValue"] | undefined;
  let formState: ReactFormState | undefined;
  let actionStatus: number | undefined;
  let temporaryReferences: unknown;
  if (renderRequest.isAction) {
    if (renderRequest.actionId === undefined) {
      // otherwise server function is called via `<form action={...}>`
      // before hydration (e.g. when javascript is disabled).
      // aka progressive enhancement.
      const formData = await request.formData();
      const decodedAction = await decodeAction(formData);
      try {
        // oxlint-disable-next-line typescript/no-confusing-void-expression -- `decodeAction` returns void per type, but the value is passed to `decodeFormState` per the RSC progressive-enhancement API
        const result = await decodedAction();
        formState = await decodeFormState(result, formData);
      } catch {
        // there's no single general obvious way to surface this error,
        // so explicitly return classic 500 response.
        return new Response("Internal Server Error: server action failed", {
          status: 500,
        });
      }
    } else {
      // action is called via `ReactClient.setServerCallback`.
      const contentType = request.headers.get("content-type");
      const body =
        contentType?.startsWith("multipart/form-data") === true
          ? await request.formData()
          : await request.text();
      temporaryReferences = createTemporaryReferenceSet();
      const args = await decodeReply(body, { temporaryReferences });
      const action = await loadServerAction(renderRequest.actionId);
      try {
        // oxlint-disable-next-line typescript/no-unsafe-assignment, typescript/no-unsafe-call -- `loadServerAction` returns an untyped `Function` from the RSC runtime
        const data = await action(...args);
        returnValue = { ok: true, data };
      } catch (error: unknown) {
        returnValue = { ok: false, data: error };
        actionStatus = 500;
      }
    }
  }

  // serialization from React VDOM tree to RSC stream.
  // we render RSC stream after handling server function request
  // so that new render reflects updated state from server function call
  // to achieve single round trip to mutate and fetch from server.
  const rscPayload: RscPayload = { root: <Root />, formState, returnValue };
  const rscOptions = { temporaryReferences };
  const rscStream = renderToReadableStream(rscPayload, rscOptions);

  // Respond RSC stream without HTML rendering as decided by `RenderRequest`
  if (renderRequest.isRsc) {
    return new Response(rscStream, {
      status: actionStatus,
      headers: {
        "content-type": "text/x-component;charset=utf-8",
      },
    });
  }

  // oxlint-disable-next-line typescript/consistent-type-imports -- Vite RSC loadModule requires `typeof import()` in type argument
  const { renderHTML } = await import.meta.viteRsc.loadModule<typeof import("./entry.ssr.tsx")>(
    "ssr",
    "index",
  );
  return renderHTML(rscStream, {
    request,
    formState,
    // allow quick simulation of javascript disabled browser
    debugNojs: renderRequest.url.searchParams.has("__nojs"),
  });
}

const app = {
  fetch(request: Request) {
    return handler(request);
  },
};

export default app;

if (import.meta.hot) {
  import.meta.hot.accept();
}
