"use server";

import { env } from "cloudflare:workers";
import { marked } from "marked";

interface KvIndex {
  keys: string[];
  commit: string;
}

export interface Entry {
  date: string;
  html: string;
  markdown?: string;
}

export async function getRandomEntry(): Promise<Entry | null> {
  const indexJson = await env.KV.get("__index");
  const index: KvIndex =
    indexJson === null
      ? { keys: [], commit: "" }
      : // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- `JSON.parse` returns `any`; KV value is known to match this shape
        (JSON.parse(indexJson) as KvIndex);
  const { keys } = index;
  if (keys.length === 0) {
    return null;
  }
  const date = keys[Math.floor(Math.random() * keys.length)];
  const markdown = await env.KV.get(date);
  if (markdown === null) {
    return null;
  }
  return {
    date,
    html: await marked(markdown),
    ...(import.meta.env.DEV && { markdown }),
  };
}
