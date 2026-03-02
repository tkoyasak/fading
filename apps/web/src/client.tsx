"use client";

import React from "react";
import type { Entry } from "./action.tsx";

interface ViewerProps {
  initialEntry: Entry | null;
  fetchEntry: () => Promise<Entry | null>;
}

export function Viewer({ initialEntry, fetchEntry }: ViewerProps) {
  const [promise, setPromise] = React.useState(() => Promise.resolve(initialEntry));
  const [isPending, startTransition] = React.useTransition();
  const [dots, setDots] = React.useState(".");

  React.useEffect(() => {
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
    <div className="flex min-h-screen flex-col items-center px-4 py-8 sm:px-6 sm:py-10">
      <article className="relative w-full max-w-prose px-4 py-4 sm:px-8 sm:py-6">
        {/* + corner frame */}
        <span aria-hidden="true" className="absolute top-0 -right-2 -left-2 h-px bg-zinc-600" />
        <span aria-hidden="true" className="absolute -right-2 bottom-0 -left-2 h-px bg-zinc-600" />
        <span aria-hidden="true" className="absolute -top-2 -bottom-2 left-0 w-px bg-zinc-600" />
        <span aria-hidden="true" className="absolute -top-2 right-0 -bottom-2 w-px bg-zinc-600" />
        <div className="transition-opacity duration-300" style={{ opacity: isPending ? 0.5 : 1 }}>
          <React.Suspense fallback={null}>
            <Content promise={promise} />
          </React.Suspense>
        </div>
      </article>
      <footer className="mt-8">
        <button
          className="cursor-pointer border-none bg-transparent text-sm leading-none text-white underline decoration-white decoration-1 underline-offset-4 select-none hover:no-underline hover:outline-2 hover:outline-offset-[5px] hover:outline-white hover:outline-dotted disabled:cursor-not-allowed disabled:no-underline disabled:opacity-50"
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
  const entry = React.use(promise);

  return entry ? (
    <main
      className="prose text-base leading-relaxed text-white"
      dangerouslySetInnerHTML={{ __html: entry.html }}
    />
  ) : (
    <p className="py-12 text-center text-sm text-white">entry not found</p>
  );
}
