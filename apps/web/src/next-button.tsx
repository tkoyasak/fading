"use client";
import { useTransition } from "react";
import { refresh } from "./action.tsx";

export function NextButton() {
  const [isPending, startTransition] = useTransition();
  return (
    <button disabled={isPending} onClick={() => startTransition(() => refresh())}>
      {isPending ? "読み込み中…" : "次の日記"}
    </button>
  );
}
