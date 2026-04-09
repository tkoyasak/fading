"use server";

import { env } from "cloudflare:workers";
import { marked } from "marked";

export type Entry = {
  date: string;
  html: string;
  markdown?: string;
};

export async function getRandomEntry(): Promise<Entry | null> {
  const indexJson = await env.KV.get("__index");
  const { keys }: { keys: string[]; commit: string } = indexJson
    ? JSON.parse(indexJson)
    : { keys: [], commit: "" };
  if (keys.length === 0) return null;
  const date = keys[Math.floor(Math.random() * keys.length)];
  const markdown = await env.KV.get(date);
  if (!markdown) return null;
  return {
    date,
    html: String(await marked(markdown)),
    ...(import.meta.env.DEV && { markdown }),
  };
}
