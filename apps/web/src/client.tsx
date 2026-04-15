"use client";

import { ArrowRight, LoaderCircle } from "lucide-react";
import React from "react";
import type { Entry } from "./action.tsx";

interface ViewerProps {
  initial: Promise<Entry | null>;
  fetchEntry: () => Promise<Entry | null>;
}

export function Viewer({ initial, fetchEntry }: ViewerProps) {
  const [entryPromise, setEntryPromise] = React.useState(initial);
  const [isPending, startTransition] = React.useTransition();

  // oxlint-disable-next-line react-perf/jsx-no-new-function-as-prop
  function handleNext() {
    startTransition(() => {
      setEntryPromise(fetchEntry());
    });
  }

  return (
    <>
      <main>
        <React.Suspense>
          <Preview entryPromise={entryPromise} />
        </React.Suspense>
      </main>
      <footer>
        <React.Suspense fallback={<LoaderCircle size={16} className="animate-spin" />}>
          <Date entryPromise={entryPromise} />
          <button type="button" disabled={isPending} onClick={handleNext}>
            next
            {isPending ? (
              <LoaderCircle size={16} className="animate-spin" />
            ) : (
              <ArrowRight size={16} className="" />
            )}
          </button>
        </React.Suspense>
      </footer>
    </>
  );
}

interface EntryProps {
  entryPromise: Promise<Entry | null>;
}

function Preview({ entryPromise }: EntryProps) {
  const entry = React.use(entryPromise);
  return (
    <React.ViewTransition>
      <article
        // oxlint-disable-next-line react-perf/jsx-no-new-object-as-prop
        dangerouslySetInnerHTML={{ __html: entry?.html ?? "" }}
      />
    </React.ViewTransition>
  );
}

function Date({ entryPromise }: EntryProps) {
  const entry = React.use(entryPromise);
  return (
    <React.ViewTransition>
      <span>{entry?.date}</span>
    </React.ViewTransition>
  );
}
