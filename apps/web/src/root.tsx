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
        <React.ViewTransition>
          <React.Suspense fallback="Loading...">
            <Viewer initial={getRandomEntry()} fetchEntry={getRandomEntry} />
          </React.Suspense>
        </React.ViewTransition>
      </body>
    </html>
  );
}
