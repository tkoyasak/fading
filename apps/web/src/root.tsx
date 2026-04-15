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
      <body className="min-h-screen">
        <Viewer initial={getRandomEntry()} fetchEntry={getRandomEntry} />
      </body>
    </html>
  );
}
