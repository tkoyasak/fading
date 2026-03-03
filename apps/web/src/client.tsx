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
    <div className="flex flex-col items-center px-6 py-8 sm:px-8 sm:py-12">
      <article className="relative w-full max-w-prose px-4 py-8 sm:px-8 sm:py-12">
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
      <footer className="mt-8 sm:mt-12">
        <button
          className="cursor-pointer border-none bg-transparent text-sm leading-none text-zinc-300 underline decoration-zinc-300 decoration-1 underline-offset-4 select-none hover:no-underline hover:outline-2 hover:outline-offset-[5px] hover:outline-zinc-300 hover:outline-dotted disabled:cursor-not-allowed disabled:no-underline disabled:opacity-50"
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
      className="prose text-base leading-loose text-zinc-200"
      dangerouslySetInnerHTML={{ __html: entry.html }}
    />
  ) : (
    <p className="py-12 text-center text-sm text-zinc-600">entry not found</p>
  );
}
