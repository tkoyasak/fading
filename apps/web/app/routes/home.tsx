import { env } from "cloudflare:workers";
import { marked } from "marked";
import type { Entry } from "../types.ts";
import { Viewer } from "../viewer.tsx";
import type { Route } from "./+types/home";

interface KvIndex {
  keys: string[];
  commit: string;
}

async function getRandomEntry(): Promise<Entry | null> {
  const indexJson = await env.KV.get<KvIndex>("__index", "json");
  const keys = indexJson?.keys ?? [];
  if (keys.length === 0) {
    return null;
  }

  const key = keys[Math.floor(Math.random() * keys.length)];
  const raw = await env.KV.get(key, "text");
  if (raw === null) {
    return null;
  }
  // x is YYYYMMDD; convert to YYYY-MM-DD for Date parsing and display
  const x = `${key.slice(0, 4)}-${key.slice(4, 6)}-${key.slice(6, 8)}`;
  const a = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][new Date(x).getDay()];

  return {
    date: `${x} ${a}`,
    html: await marked(raw, { gfm: false }),
  };
}

export async function loader() {
  return { entry: await getRandomEntry() };
}

export function ServerComponent({ loaderData }: Route.ServerComponentProps) {
  return <Viewer entry={loaderData.entry} />;
}
