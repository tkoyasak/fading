"use client";

import React from "react";
import type { Entry } from "./action.tsx";

interface ViewerProps {
  initial: Entry | null;
  fetchEntry: () => Promise<Entry | null>;
}

export function Viewer({ initial, fetchEntry }: ViewerProps) {
  const [entry, setEntry] = React.useState(initial);
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
  function nextAction() {
    startTransition(async () => {
      const [next] = await Promise.all([
        fetchEntry(),
        new Promise((resolve) => {
          setTimeout(resolve, 1000);
        }),
      ]);
      setEntry(next);
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
          <Content entry={entry} />
        </div>
      </article>
      <footer className="mt-8 sm:mt-12">
        <button
          className="cursor-pointer border-none bg-transparent text-base leading-none text-zinc-300 underline decoration-zinc-300 decoration-1 underline-offset-4 select-none hover:no-underline hover:outline-2 hover:outline-offset-[5px] hover:outline-zinc-300 hover:outline-dotted disabled:cursor-not-allowed disabled:no-underline disabled:opacity-50"
          disabled={isPending}
          onClick={nextAction}
        >
          {isPending ? dots : "next"}
        </button>
      </footer>
      {import.meta.env.DEV && <RawMarkdown entry={entry} />}
    </div>
  );
}

interface ContentProps {
  entry: Entry | null;
}

function Content({ entry }: ContentProps) {
  return entry ? (
    <main className="prose text-base leading-loose text-zinc-200" dangerouslySetInnerHTML={entry} />
  ) : (
    <p className="py-12 text-center text-base text-zinc-200">entry not found</p>
  );
}

function RawMarkdown({ entry }: ContentProps) {
  return entry?.__raw === undefined ? null : (
    <pre className="wrap-break-words mt-8 w-full max-w-prose overflow-x-auto font-mono text-xs whitespace-pre-wrap text-zinc-400 sm:mt-12">
      {entry.__raw}
    </pre>
  );
}
