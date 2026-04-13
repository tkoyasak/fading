"use server";

import { env } from "cloudflare:workers";
import { marked } from "marked";

interface KvIndex {
  keys: string[];
  commit: string;
}

export interface Entry {
  date: string;
  __html: string;
  __raw?: string;
}

export async function getRandomEntry(): Promise<Entry | null> {
  const indexJson = await env.KV.get<KvIndex>("__index", "json");
  const keys = indexJson?.keys ?? [];
  if (keys.length === 0) {
    return null;
  }
  const date = keys[Math.floor(Math.random() * keys.length)];
  const raw = await env.KV.get(date, "text");
  if (raw === null) {
    return null;
  }
  return {
    date,
    __html: await marked(raw),
    ...(import.meta.env.DEV && { __raw: raw }),
  };
}
