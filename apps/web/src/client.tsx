"use client";

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
      setEntryPromise(
        new Promise((resolve) => {
          setTimeout(resolve, 1104);
          // oxlint-disable-next-line promise/prefer-await-to-then
        }).then(fetchEntry),
      );
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
        <span>{entry?.date}</span>
      </React.ViewTransition>
      <button
        type="button"
        disabled={isPending}
        onClick={onClick}
        className="flex items-center gap-1"
      >
        <React.ViewTransition>
          <AudioLinesIcon size={24} animating={isPending} />
        </React.ViewTransition>
      </button>
    </div>
  );
}

function FallbackFooter() {
  return (
    <div className="flex justify-end">
      <AudioLinesIcon size={24} animating />
    </div>
  );
}

interface AudioLinesIconProps {
  size: number;
  animating: boolean;
}

const BAR_COUNT = 8;

function AudioLinesIcon({ size, animating }: AudioLinesIconProps) {
  const [isAnimating, setIsAnimating] = React.useState(animating);
  const completedRef = React.useRef(0);

  React.useEffect(() => {
    if (animating) {
      setIsAnimating(true);
      completedRef.current = 0;
    }
  }, [animating]);

  // oxlint-disable-next-line react-perf/jsx-no-new-function-as-prop
  function handleIteration() {
    if (!animating) {
      completedRef.current += 1;
      if (completedRef.current > BAR_COUNT) {
        React.startTransition(() => {
          setIsAnimating(false);
          completedRef.current = 0;
        });
      }
    }
  }

  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="currentColor"
      className={isAnimating ? "audio-lines animating" : "audio-lines"}
      onAnimationIteration={handleIteration}
    >
      <rect x="1" width="2" rx="1" />
      <rect x="5" width="2" rx="1" />
      <rect x="9" width="2" rx="1" />
      <rect x="13" width="2" rx="1" />
      <rect x="17" width="2" rx="1" />
      <rect x="21" width="2" rx="1" />
      {isAnimating && (
        <>
          <rect className="bar-ping" x="1" width="2" rx="1" />
          <rect className="bar-ping" x="5" width="2" rx="1" />
          <rect className="bar-ping" x="9" width="2" rx="1" />
          <rect className="bar-ping" x="13" width="2" rx="1" />
          <rect className="bar-ping" x="17" width="2" rx="1" />
          <rect className="bar-ping" x="21" width="2" rx="1" />
        </>
      )}
    </svg>
  );
}
