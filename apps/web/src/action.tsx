"use server";

import { env } from "cloudflare:workers";
import { marked } from "marked";

export type Entry = { date: string; html: string };

export async function getRandomEntry(): Promise<Entry | null> {
  const indexJson = await env.KV.get("__index");
  const keys: string[] = indexJson ? JSON.parse(indexJson) : [];
  if (keys.length === 0) return null;
  const date = keys[Math.floor(Math.random() * keys.length)];
  const markdown = await env.KV.get(date);
  if (!markdown) return null;
  return { date, html: String(await marked(markdown)) };
}
