import { component$, useSignal, useTask$, $, type Signal } from "@qwik.dev/core";
import { routeLoader$, server$ } from "@qwik.dev/router";
import type { DocumentHead } from "@qwik.dev/router";
import { marked } from "marked";

interface KvIndex {
  keys: string[];
}

interface Entry {
  date: string;
  html: string;
}

async function loadRandomEntry(kv: KVNamespace): Promise<Entry | null> {
  const indexData = await kv.get<KvIndex>("__index", "json");
  const keys = indexData?.keys ?? [];
  if (keys.length === 0) return null;

  const key = keys[Math.floor(Math.random() * keys.length)];
  if (key === undefined) return null;

  const raw = await kv.get(key, "text");
  if (raw === null) return null;

  const x = `${key.slice(0, 4)}-${key.slice(4, 6)}-${key.slice(6, 8)}`;
  const a = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][new Date(x).getDay()];

  return {
    date: `${x} ${a}`,
    html: await marked(raw, { gfm: false }),
  };
}

function getKv(platform: unknown): KVNamespace | null {
  const p = platform as { env?: Env };
  return p?.env?.KV ?? null;
}

export const useEntry = routeLoader$(async (event) => {
  const kv = getKv(event.platform);
  if (!kv) return null;
  return loadRandomEntry(kv);
});

const fetchNewEntry = server$(async function () {
  const kv = getKv(this.platform);
  if (!kv) return null;
  return loadRandomEntry(kv);
});

const BAR_COUNT = 8;

const AudioLinesIcon = component$<{ size: number; animating: Signal<boolean> }>((props) => {
  const isAnimating = useSignal(props.animating.value);
  const completedCount = useSignal(0);

  useTask$(({ track }) => {
    const anim = track(() => props.animating.value);
    if (anim) {
      isAnimating.value = true;
      completedCount.value = 0;
    }
  });

  return (
    <svg
      width={props.size}
      height={props.size}
      viewBox="0 0 24 24"
      fill="currentColor"
      class={isAnimating.value ? "audio-lines animating" : "audio-lines"}
      onAnimationIteration$={() => {
        if (!props.animating.value) {
          completedCount.value += 1;
          if (completedCount.value > BAR_COUNT) {
            isAnimating.value = false;
            completedCount.value = 0;
          }
        }
      }}
    >
      <rect x="1" width="2" rx="1" />
      <rect x="5" width="2" rx="1" />
      <rect x="9" width="2" rx="1" />
      <rect x="13" width="2" rx="1" />
      <rect x="17" width="2" rx="1" />
      <rect x="21" width="2" rx="1" />
      {isAnimating.value && (
        <>
          <rect class="bar-ping" x="1" width="2" rx="1" />
          <rect class="bar-ping" x="5" width="2" rx="1" />
          <rect class="bar-ping" x="9" width="2" rx="1" />
          <rect class="bar-ping" x="13" width="2" rx="1" />
          <rect class="bar-ping" x="17" width="2" rx="1" />
          <rect class="bar-ping" x="21" width="2" rx="1" />
        </>
      )}
    </svg>
  );
});

export default component$(() => {
  const entry = useEntry();
  const currentDate = useSignal(entry.value?.date ?? "");
  const currentHtml = useSignal(entry.value?.html ?? "");
  const isPending = useSignal(false);

  const handleNext = $(async () => {
    if (isPending.value) return;
    isPending.value = true;
    await new Promise<void>((resolve) => setTimeout(resolve, 1104));
    try {
      const newEntry = await fetchNewEntry();
      if (newEntry) {
        currentDate.value = newEntry.date;
        currentHtml.value = newEntry.html;
      }
    } finally {
      isPending.value = false;
    }
  });

  return (
    <>
      <main class="mx-auto max-w-sm pb-6">
        <article class="prose" dangerouslySetInnerHTML={currentHtml.value} />
      </main>
      <footer class="fixed right-0 bottom-0 left-0 text-[16px] backdrop-blur-[1px]">
        <div class="mx-auto max-w-sm">
          <div class="flex items-center justify-between">
            <div class="flex lining-nums tabular-nums">
              <span>{currentDate.value[0]}</span>
              <span>{currentDate.value[1]}</span>
              <span>{currentDate.value[2]}</span>
              <span>{currentDate.value[3]}</span>
              <span>-</span>
              <span>{currentDate.value[5]}</span>
              <span>{currentDate.value[6]}</span>
              <span>-</span>
              <span>{currentDate.value[8]}</span>
              <span>{currentDate.value[9]}</span>
              <span class="ml-1">
                {currentDate.value[11]}
                {currentDate.value[12]}
                {currentDate.value[13]}
              </span>
            </div>
            <button
              type="button"
              disabled={isPending.value}
              onClick$={handleNext}
              class="flex items-center gap-1"
            >
              <AudioLinesIcon size={24} animating={isPending} />
            </button>
          </div>
        </div>
      </footer>
    </>
  );
});

export const head: DocumentHead = {
  title: "fading",
};
