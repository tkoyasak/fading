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

function getKv(platform: unknown): KVNamespace {
  const p = platform as { env?: Env };
  const kv = p?.env?.KV;
  if (!kv) throw "KV namespace not available";
  return kv;
}

async function loadRandomEntry(platform: unknown): Promise<Entry> {
  try {
    const kv = getKv(platform);
    const indexData = await kv.get<KvIndex>("__index", "json");
    const keys = indexData?.keys ?? [];
    if (keys.length === 0) throw "No entries in KV";

    const key = keys[Math.floor(Math.random() * keys.length)];
    if (key === undefined) throw "No entry key found";

    const raw = await kv.get(key, "text");
    if (raw === null) throw `Entry not found: ${key}`;

    const x = `${key.slice(0, 4)}-${key.slice(4, 6)}-${key.slice(6, 8)}`;
    const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const day = days[new Date(x).getDay()];
    if (day === undefined) throw `Invalid date: ${x}`;

    return { date: `${x} ${day}`, html: await marked(raw, { gfm: false }) };
  } catch (e) {
    return { date: "", html: String(e) };
  }
}

export const useEntry = routeLoader$(async (event) => {
  return loadRandomEntry(event.platform);
});

const fetchNewEntry = server$(async function () {
  return loadRandomEntry(this.platform);
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
  const currentDate = useSignal(entry.value.date);
  const currentHtml = useSignal(entry.value.html);
  const isPending = useSignal(false);

  const handleNext = $(async () => {
    if (isPending.value) return;
    isPending.value = true;
    await new Promise((resolve) => setTimeout(resolve, 1104));
    const newEntry = await fetchNewEntry();
    currentDate.value = newEntry.date;
    currentHtml.value = newEntry.html;
    isPending.value = false;
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
