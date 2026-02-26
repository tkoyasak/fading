"use client";

import { useState, useTransition } from "react";
import { getRandomEntry } from "./action.tsx";
import type { Entry } from "./action.tsx";

export function DiaryViewer({ initialEntry }: { initialEntry: Entry | null }) {
  const [entry, setEntry] = useState(initialEntry);
  const [isPending, startTransition] = useTransition();

  function handleNext() {
    startTransition(async () => {
      const newEntry = await getRandomEntry();
      setEntry(newEntry);
    });
  }

  return (
    <>
      {entry ? (
        <>
          <header>
            <time dateTime={entry.date}>{entry.date}</time>
          </header>
          <main dangerouslySetInnerHTML={{ __html: entry.html }} />
          <footer>
            <button disabled={isPending} onClick={handleNext}>
              {isPending ? "読み込み中…" : "次の日記"}
            </button>
          </footer>
        </>
      ) : (
        <p>日記が見つかりませんでした。</p>
      )}
    </>
  );
}
