import { $, component$, type Signal, useSignal, useVisibleTask$ } from "@qwik.dev/core";
import { type DocumentHead, routeLoader$, server$ } from "@qwik.dev/router";
import { marked } from "marked";
import { createErr, createOk, isOk, type Result, unwrapErr, unwrapOk } from "option-t/plain_result";

interface KvIndex {
  keys: string[];
}

interface Entry {
  date: string;
  day: string;
  html: string;
}

declare global {
  interface QwikRouterPlatform {
    env: Env;
  }
}

const DOW = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] as const;

async function loadRandomEntry(platform: QwikRouterPlatform): Promise<Result<Entry, string>> {
  const kv = platform.env?.KV;
  if (!kv) return createErr("KV namespace not available");

  const indexData = await kv.get<KvIndex>("__index", "json");
  const keys = indexData?.keys ?? [];
  if (keys.length === 0) return createErr("No entries in KV");

  const key = keys[Math.floor(Math.random() * keys.length)];
  if (key === undefined) return createErr("No entry key found");

  const raw = await kv.get(key, "text");
  if (raw === null) return createErr(`Entry not found: ${key}`);

  const date = `${key.slice(0, 4)}-${key.slice(4, 6)}-${key.slice(6, 8)}`;
  const day = DOW[new Date(date).getUTCDay()];
  if (day === undefined) return createErr(`Invalid date: ${date}`);

  return createOk({ date, day, html: await marked(raw, { gfm: false }) });
}

export const useEntry = routeLoader$(async (event) => {
  return loadRandomEntry(event.platform);
});

const fetchNewEntry = server$(async function () {
  return loadRandomEntry(this.platform);
});

const NOISE_COLORS = [
  [0x00, 0x09, 0xf3], // blue
  [0xff, 0x50, 0x31], // orange
  [0xfe, 0xef, 0x00], // yellow
  [0xdf, 0x06, 0x71], // pink
  [0x5c, 0xff, 0x0b], // light-green
  [0x00, 0x9e, 0xe3], // light-blue
] as const;

const TRANSITION_MS = 1104;

const NoiseCanvas = component$<{ visible: Signal<boolean>; animating: Signal<boolean> }>(
  (props) => {
    const canvasRef = useSignal<HTMLCanvasElement>();

    // useVisibleTask$ is required: canvas + requestAnimationFrame is a client-only loop.
    // oxlint-disable-next-line qwik/no-use-visible-task
    useVisibleTask$(
      ({ cleanup }) => {
        const canvas = canvasRef.value;
        if (!canvas) return;

        const SKIP = 3;
        const TTL = 5;
        const SPAWN_RATE = 0.0005;
        const SQUARE_RATE = 0.005;

        let W = 0;
        let H = 0;
        let ctx: CanvasRenderingContext2D | null = null;
        let img: ImageData | null = null;
        let life = new Uint8Array(0);
        let color = new Uint8Array(0);

        const resize = () => {
          canvas.width = Math.round(window.innerWidth / 4);
          canvas.height = Math.round(window.innerHeight / 4);
          W = canvas.width;
          H = canvas.height;
          ctx = canvas.getContext("2d");
          if (!ctx) return;
          img = ctx.getImageData(0, 0, W, H);
          life = new Uint8Array(W * H);
          color = new Uint8Array(W * H);
        };
        resize();
        window.addEventListener("resize", resize);

        let rafId = 0;
        let cancelled = false;
        let frame = 0;

        // Poll visible inside the loop instead of tracking it: tracking would invoke cleanup
        // on visible: true -> false and cancel rAF mid-fadeout. Let the loop run itself until
        // hasAlive becomes false. Writes to props.animating expose progress to the parent.
        const loop = () => {
          if (cancelled || !ctx || !img) return;

          if (frame % SKIP === 0) {
            const active = props.visible.value;

            if (active && !props.animating.value) {
              props.animating.value = true;
            }

            if (props.animating.value) {
              if (active) {
                const spawns = Math.round(W * H * SPAWN_RATE);
                for (let i = 0; i < spawns; i++) {
                  const p = (Math.random() * W * H) | 0;
                  const x = p % W;
                  const y = (p / W) | 0;
                  const c = (Math.random() * NOISE_COLORS.length) | 0;
                  if (Math.random() < SQUARE_RATE && x + 1 < W && y + 1 < H) {
                    for (let dy = 0; dy < 2; dy++) {
                      for (let dx = 0; dx < 2; dx++) {
                        const q = (y + dy) * W + (x + dx);
                        life[q] = TTL;
                        color[q] = c;
                      }
                    }
                  } else {
                    life[p] = TTL;
                    color[p] = c;
                  }
                }
              }

              let dirty = false;
              let hasAlive = false;
              for (let p = 0; p < W * H; p++) {
                const i = p * 4;
                const l = life[p]!;
                if (l > 0) {
                  life[p] = l - 1;
                  const c = NOISE_COLORS[color[p]!]!;
                  img.data[i] = c[0];
                  img.data[i + 1] = c[1];
                  img.data[i + 2] = c[2];
                  img.data[i + 3] = 255;
                  dirty = true;
                  hasAlive = true;
                } else if (img.data[i + 3]! !== 0) {
                  img.data[i + 3] = 0;
                  dirty = true;
                }
              }
              if (dirty) ctx.putImageData(img, 0, 0);
              if (!active && !hasAlive) {
                props.animating.value = false;
              }
            }
          }
          frame++;
          rafId = requestAnimationFrame(loop);
        };
        loop();

        cleanup(() => {
          cancelled = true;
          cancelAnimationFrame(rafId);
          window.removeEventListener("resize", resize);
        });
      },
      { strategy: "document-ready" },
    );

    return (
      <canvas
        ref={canvasRef}
        class="fixed inset-0 z-50 h-screen w-screen [image-rendering:pixelated] pointer-events-none"
      />
    );
  },
);

export default component$(() => {
  const entry = useEntry();
  const currentResult = useSignal<Result<Entry, string>>(entry.value);
  const isPending = useSignal(false);
  const isAnimating = useSignal(false);

  const handleNext = $(async () => {
    if (isAnimating.value || isPending.value) return;
    isPending.value = true;
    await new Promise((resolve) => setTimeout(resolve, TRANSITION_MS));
    currentResult.value = await fetchNewEntry();
    isPending.value = false;
  });

  const r = currentResult.value;
  const ok = isOk(r) ? unwrapOk(r) : null;

  return (
    <>
      <NoiseCanvas visible={isPending} animating={isAnimating} />
      <main class="mx-auto max-w-sm pb-6">
        {ok ? (
          <article class="prose" dangerouslySetInnerHTML={ok.html} />
        ) : (
          <article class="prose">
            <p>{unwrapErr(r)}</p>
          </article>
        )}
      </main>
      <footer class="fixed right-0 bottom-0 left-0 text-[16px] backdrop-blur-[1px]">
        <div class="mx-auto max-w-sm">
          <div class="flex items-center justify-between">
            <div class="flex lining-nums tabular-nums">
              {ok && (
                <>
                  {Array.from(ok.date).map((c, i) => (
                    <span key={i}>{c}</span>
                  ))}
                  <span class="ml-1">{ok.day}</span>
                </>
              )}
            </div>
            <button
              type="button"
              disabled={isAnimating.value || isPending.value}
              onClick$={handleNext}
            >
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
