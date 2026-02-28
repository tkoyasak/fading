import { Suspense } from "react";
import { getRandomEntry } from "./action.tsx";
import { Viewer } from "./client.tsx";

export async function Root() {
  const initialEntry = await getRandomEntry();

  return (
    <html lang="ja">
      <head>
        <meta charSet="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>fading</title>
      </head>
      <body className="bg-zinc-950 text-zinc-100 min-h-screen font-sans antialiased">
        <Suspense fallback={<p>Loading...</p>}>
          <Viewer initialEntry={initialEntry} fetchEntry={getRandomEntry} />
        </Suspense>
      </body>
    </html>
  );
}
