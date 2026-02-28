"use client";

import { use, useState, useTransition } from "react";
import type { Entry } from "./action.tsx";

interface ViewerProps {
  initialEntry: Entry | null;
  fetchEntry: () => Promise<Entry | null>;
}

export function Viewer({ initialEntry, fetchEntry }: ViewerProps) {
  const [promise, setPromise] = useState(() => Promise.resolve(initialEntry));
  const [isPending, startTransition] = useTransition();

  function handleNext() {
    startTransition(() =>
      setPromise(
        Promise.all([fetchEntry(), new Promise((r) => setTimeout(r, 800))]).then(
          ([entry]) => entry,
        ),
      ),
    );
  }

  return (
    <div className="min-h-screen flex flex-col items-center justify-center px-6 py-16">
      <article
        className="max-w-prose w-full transition-opacity duration-300"
        style={{ opacity: isPending ? 0.4 : 1 }}
      >
        <Content promise={promise} />
      </article>
      <footer className="mt-12">
        <button
          className="size-10 flex items-center justify-center rounded-full bg-zinc-800 hover:bg-zinc-700 text-zinc-400 hover:text-zinc-200 transition-all border-none cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
          disabled={isPending}
          onClick={handleNext}
          aria-label="次の日記"
        >
          <span className="icon-[material-symbols--add] size-5" />
        </button>
      </footer>
    </div>
  );
}

interface ContentProps {
  promise: Promise<Entry | null>;
}

function Content({ promise }: ContentProps) {
  const entry = use(promise);

  return entry ? (
    <>
      <header className="mb-8">
        <time className="text-xs text-zinc-600 font-mono tracking-widest" dateTime={entry.date}>
          {entry.date}
        </time>
      </header>
      <main
        className="prose text-zinc-300 leading-relaxed text-base"
        dangerouslySetInnerHTML={{ __html: entry.html }}
      />
    </>
  ) : (
    <p className="text-zinc-600 text-sm text-center py-12">日記が見つかりませんでした。</p>
  );
}
