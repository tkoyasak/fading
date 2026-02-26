import { getRandomEntry } from "./action.tsx";
import { DiaryViewer } from "./client.tsx";

export async function Root() {
  const initialEntry = await getRandomEntry();
  return (
    <html lang="ja">
      <head>
        <meta charSet="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>fading</title>
      </head>
      <body>
        <DiaryViewer initialEntry={initialEntry} />
      </body>
    </html>
  );
}
