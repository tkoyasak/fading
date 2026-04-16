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
      <main className="mx-auto max-w-sm pb-6">
        <React.Suspense>
          <Preview entryPromise={entryPromise} />
        </React.Suspense>
      </main>
      <footer className="fixed right-0 bottom-0 left-0 text-[16px] backdrop-blur-[1px]">
        <div className="mx-auto max-w-sm">
          <React.ViewTransition>
            <React.Suspense
              // oxlint-disable-next-line react-perf/jsx-no-jsx-as-prop
              fallback={<FallbackFooter />}
            >
              <Footer entryPromise={entryPromise} isPending={isPending} onClick={handleNext} />
            </React.Suspense>
          </React.ViewTransition>
        </div>
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
        className="prose"
        // oxlint-disable-next-line react-perf/jsx-no-new-object-as-prop
        dangerouslySetInnerHTML={{ __html: entry?.html ?? "" }}
      />
    </React.ViewTransition>
  );
}

interface FooterProps extends EntryProps {
  isPending: boolean;
  onClick: () => void;
}

function Footer({ entryPromise, isPending, onClick }: FooterProps) {
  const entry = React.use(entryPromise);
  return (
    <div className="flex items-center justify-between">
      <React.ViewTransition>
        <span className="tabular-nums">{entry?.date}</span>
      </React.ViewTransition>
      <button
        type="button"
        disabled={isPending}
        onClick={onClick}
        className="flex items-center gap-1"
      >
        <span className="leading-none [text-box-edge:cap_alphabetic] [text-box-trim:trim-both]">
          next
        </span>
        <React.ViewTransition>
          {isPending ? (
            <LoaderCircle size={16} className="animate-spin" />
          ) : (
            <ArrowRight size={16} />
          )}
        </React.ViewTransition>
      </button>
    </div>
  );
}

function FallbackFooter() {
  return (
    <div className="flex justify-end">
      <LoaderCircle size={16} className="animate-spin" />
    </div>
  );
}
