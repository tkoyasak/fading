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
  const entry = React.use(entryPromise);

  // oxlint-disable-next-line react-perf/jsx-no-new-function-as-prop
  function handleNext() {
    startTransition(() => {
      setEntryPromise(fetchEntry());
    });
  }

  return (
    <div>
      <Preview content={entry.html} />
      <Footer date={entry.date} isPending={isPending} onClick={handleNext} />
    </div>
  );
}

interface PreviewProps {
  content: string;
}

function Preview({ content }: PreviewProps) {
  return (
    <div>
      <React.ViewTransition>
        <div
          // oxlint-disable-next-line react-perf/jsx-no-new-object-as-prop
          dangerouslySetInnerHTML={{
            __html: content,
          }}
        />
      </React.ViewTransition>
    </div>
  );
}

interface FooterProps {
  date: string;
  isPending: boolean;
  onClick: () => void;
}

function Footer({ date, isPending, onClick }: FooterProps) {
  return (
    <div>
      <React.ViewTransition>
        <div>{date}</div>
      </React.ViewTransition>
      <React.ViewTransition>
        <button disabled={isPending} onClick={onClick}>
          next
        </button>
      </React.ViewTransition>
    </div>
  );
}
