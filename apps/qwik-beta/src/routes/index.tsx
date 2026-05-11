import { component$, useSignal, useVisibleTask$, $, type Signal } from "@qwik.dev/core";
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

const NoiseCanvas = component$<{ visible: Signal<boolean> }>((props) => {
  const canvasRef = useSignal<HTMLCanvasElement>();

  // oxlint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(
    ({ track, cleanup }) => {
      const isVisible = track(() => props.visible.value);
      const canvas = canvasRef.value;
      if (!canvas) return;

      const ctx = canvas.getContext("2d");
      if (!ctx) return;

      const W = canvas.width;
      const H = canvas.height;
      const img = ctx.createImageData(W, H);
      let rafId: number | undefined;

      if (isVisible) {
        const loop = () => {
          for (let i = 0; i < img.data.length; i += 4) {
            img.data[i] = Math.random() * 255;
            img.data[i + 1] = Math.random() * 255;
            img.data[i + 2] = Math.random() * 255;
            img.data[i + 3] = 255;
          }
          ctx.putImageData(img, 0, 0);
          rafId = requestAnimationFrame(loop);
        };
        loop();
      }

      cleanup(() => {
        if (rafId !== undefined) cancelAnimationFrame(rafId);
      });
    },
    { strategy: "document-ready" },
  );

  return (
    <canvas
      ref={canvasRef}
      width={200}
      height={150}
      class={[
        "fixed inset-0 z-50 h-screen w-screen [image-rendering:pixelated]",
        props.visible.value ? "" : "hidden",
      ]}
    />
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
      <NoiseCanvas visible={isPending} />
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
            <button type="button" disabled={isPending.value} onClick$={handleNext}>
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="1em"
                height="1em"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="lucide lucide-astroid-icon lucide-astroid text-[#0009F3]"
              >
                <path d="M12.983 21.186a1 1 0 0 1-1.966 0 10 10 0 0 0-8.203-8.203 1 1 0 0 1 0-1.966 10 10 0 0 0 8.203-8.203 1 1 0 0 1 1.966 0 10 10 0 0 0 8.203 8.203 1 1 0 0 1 0 1.966 10 10 0 0 0-8.203 8.203" />
              </svg>
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
