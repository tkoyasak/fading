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
    let id: ReturnType<typeof setInterval> | undefined;
    if (isPending) {
      id = setInterval(() => {
        setDots((d) => (d.length === 3 ? "." : `${d}.`));
      }, 400);
    } else {
      setDots(".");
    }
    return () => {
      clearInterval(id);
    };
  }, [isPending]);

  // oxlint-disable-next-line react-perf/jsx-no-new-function-as-prop -- DOM `button` element; `useCallback` wouldn't prevent re-renders since button is not a memoized component
  function handleNext() {
    startTransition(() => {
      setPromise(
        Promise.all([
          fetchEntry(),
          new Promise<void>((resolve) => {
            setTimeout(resolve, 800);
          }),
          // oxlint-disable-next-line promise/prefer-await-to-then -- extracts the first element from Promise.all; using `await` would delay `setPromise` and change Suspense timing
        ]).then(([entry]) => entry),
      );
    });
  }

  return (
    <div className="flex flex-col items-center px-6 py-8 sm:px-8 sm:py-12">
      <article className="relative w-full max-w-prose px-4 py-8 sm:px-8 sm:py-12">
        {/* + corner frame */}
        <span aria-hidden="true" className="absolute top-0 -right-2 -left-2 h-px bg-zinc-600" />
        <span aria-hidden="true" className="absolute -right-2 bottom-0 -left-2 h-px bg-zinc-600" />
        <span aria-hidden="true" className="absolute -top-2 -bottom-2 left-0 w-px bg-zinc-600" />
        <span aria-hidden="true" className="absolute -top-2 right-0 -bottom-2 w-px bg-zinc-600" />
        <div
          className={`transition-opacity duration-300 ${isPending ? "opacity-50" : "opacity-100"}`}
        >
          <React.Suspense fallback={null}>
            <Content promise={promise} />
          </React.Suspense>
        </div>
      </article>
      <footer className="mt-8 sm:mt-12">
        <button
          className="cursor-pointer border-none bg-transparent text-base leading-none text-zinc-300 underline decoration-zinc-300 decoration-1 underline-offset-4 select-none hover:no-underline hover:outline-2 hover:outline-offset-[5px] hover:outline-zinc-300 hover:outline-dotted disabled:cursor-not-allowed disabled:no-underline disabled:opacity-50"
          disabled={isPending}
          onClick={handleNext}
        >
          {isPending ? dots : "next"}
        </button>
      </footer>
      {import.meta.env.DEV && (
        <React.Suspense fallback={null}>
          <RawMarkdown promise={promise} />
        </React.Suspense>
      )}
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
      // oxlint-disable-next-line react-perf/jsx-no-new-object-as-prop -- `dangerouslySetInnerHTML` requires a plain object per React API; extracting to a variable doesn't help since `entry.html` changes each render
      dangerouslySetInnerHTML={{ __html: entry.html }}
    />
  ) : (
    <p className="py-12 text-center text-base text-zinc-200">entry not found</p>
  );
}

function RawMarkdown({ promise }: ContentProps) {
  const entry = React.use(promise);

  return entry?.markdown === undefined ? null : (
    <pre className="wrap-break-words mt-8 w-full max-w-prose overflow-x-auto font-mono text-xs whitespace-pre-wrap text-zinc-400 sm:mt-12">
      {entry.markdown}
    </pre>
  );
}
