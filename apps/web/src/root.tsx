import { env } from "cloudflare:workers";
import { marked } from "marked";
import { NextButton } from "./next-button.tsx";

export async function Root() {
  const indexJson = await env.KV.get("__index");
  const keys: string[] = indexJson ? JSON.parse(indexJson) : [];

  const date = keys.length > 0 ? keys[Math.floor(Math.random() * keys.length)] : null;

  const markdown = date ? await env.KV.get(date) : null;
  const html = markdown ? String(await marked(markdown)) : null;

  return (
    <html lang="ja">
      <head>
        <meta charSet="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>fading</title>
      </head>
      <body>
        {date && html ? (
          <>
            <header>
              <time dateTime={date}>{date}</time>
            </header>
            <main dangerouslySetInnerHTML={{ __html: html }} />
            <footer>
              <NextButton />
            </footer>
          </>
        ) : (
          <p>日記が見つかりませんでした。</p>
        )}
      </body>
    </html>
  );
}
