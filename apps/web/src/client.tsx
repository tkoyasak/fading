"use client";

import { Suspense, use, useEffect, useState, useTransition } from "react";
import type { Entry } from "./action.tsx";

interface ViewerProps {
  initialEntry: Entry | null;
  fetchEntry: () => Promise<Entry | null>;
}

export function Viewer({ initialEntry, fetchEntry }: ViewerProps) {
  const [promise, setPromise] = useState(() => Promise.resolve(initialEntry));
  const [isPending, startTransition] = useTransition();
  const [dots, setDots] = useState(".");

  useEffect(() => {
    if (!isPending) {
      setDots(".");
      return;
    }
    const id = setInterval(() => setDots((d) => (d.length === 3 ? "." : d + ".")), 400);
    return () => clearInterval(id);
  }, [isPending]);

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
    <div className="min-h-screen flex flex-col items-center px-6 py-10">
      <article
        className="relative max-w-prose w-full px-8 py-6 transition-opacity duration-300"
        style={{ opacity: isPending ? 0.4 : 1 }}
      >
        {/* + corner frame */}
        <span aria-hidden="true" className="absolute top-0 -left-2 -right-2 h-px bg-zinc-600" />
        <span aria-hidden="true" className="absolute bottom-0 -left-2 -right-2 h-px bg-zinc-600" />
        <span aria-hidden="true" className="absolute left-0 -top-2 -bottom-2 w-px bg-zinc-600" />
        <span aria-hidden="true" className="absolute right-0 -top-2 -bottom-2 w-px bg-zinc-600" />
        <Suspense fallback={null}>
          <Content promise={promise} />
        </Suspense>
      </article>
      <footer className="mt-8">
        <button
          className="px-2 py-1 text-sm text-zinc-300 hover:text-zinc-300 underline underline-offset-4 decoration-1 decoration-zinc-300 disabled:no-underline hover:no-underline hover:outline-dotted hover:outline-zinc-400 bg-transparent border-none cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed transition-all"
          disabled={isPending}
          onClick={handleNext}
        >
          {isPending ? dots : "next"}
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
    <main
      className="prose text-zinc-300 leading-relaxed text-base text-left"
      dangerouslySetInnerHTML={{ __html: entry.html }}
    />
  ) : (
    <p className="text-zinc-600 text-sm text-center py-12">日記が見つかりませんでした。</p>
  );
}
