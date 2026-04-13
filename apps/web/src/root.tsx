import React from "react";
import { getRandomEntry } from "./action.tsx";
import { Viewer } from "./client.tsx";

export function Root() {
  return (
    <html lang="ja">
      <head>
        <meta charSet="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>fading</title>
      </head>
      <body className="min-h-screen bg-zinc-950 font-sans text-zinc-100 antialiased">
        <React.Suspense fallback="Loading...">
          <InitialView />
        </React.Suspense>
      </body>
    </html>
  );
}

async function InitialView() {
  const initial = await getRandomEntry();
  return <Viewer initial={initial} fetchEntry={getRandomEntry} />;
}
