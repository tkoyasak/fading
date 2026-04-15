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
}

export async function getRandomEntry(): Promise<Entry | null> {
  const indexJson = await env.KV.get<KvIndex>("__index", "json");
  const keys = indexJson?.keys ?? [];
  if (keys.length === 0) {
    return null;
  }

  const x = keys[Math.floor(Math.random() * keys.length)];
  const raw = await env.KV.get(x, "text");
  if (raw === null) {
    return null;
  }
  const a = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][new Date(x).getDay()];

  return {
    date: `${x} ${a}`,
    html: await marked(raw),
  };
}
