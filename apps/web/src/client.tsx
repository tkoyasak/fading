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
    <>
      <Content promise={promise} />
      <footer>
        <button disabled={isPending} onClick={handleNext}>
          次の日記
        </button>
      </footer>
    </>
  );
}

interface ContentProps {
  promise: Promise<Entry | null>;
}

function Content({ promise }: ContentProps) {
  const entry = use(promise);

  return entry ? (
    <>
      <header>
        <time dateTime={entry.date}>{entry.date}</time>
      </header>
      <main dangerouslySetInnerHTML={{ __html: entry.html }} />
    </>
  ) : (
    <p>日記が見つかりませんでした。</p>
  );
}
