"use client";

import React from "react";
import { useRevalidator } from "react-router";

interface ViewerProps {
  html: string;
  date: string;
}

export function Viewer({ html, date }: ViewerProps) {
  const revalidator = useRevalidator();
  const [isPending, startTransition] = React.useTransition();
  // isPending covers the sleep inside the transition; revalidator.state covers the
  // loader refetch that runs after the sleep. Together they span the full round-trip.
  const showPending = isPending || revalidator.state === "loading";

  // oxlint-disable-next-line react-perf/jsx-no-new-function-as-prop
  function handleNext() {
    startTransition(async () => {
      await new Promise((resolve) => {
        setTimeout(resolve, 1104);
      });
      // https://react.dev/reference/react/useTransition#react-doesnt-treat-my-state-update-after-await-as-a-transition
      React.startTransition(() => {
        void revalidator.revalidate();
      });
    });
  }

  return (
    <>
      <main className="mx-auto max-w-sm pb-6">
        <Preview html={html} />
      </main>
      <footer className="fixed right-0 bottom-0 left-0 text-[16px] backdrop-blur-[1px]">
        <div className="mx-auto max-w-sm">
          <React.ViewTransition>
            <Footer date={date} isPending={showPending} onClick={handleNext} />
          </React.ViewTransition>
        </div>
      </footer>
    </>
  );
}

interface PreviewProps {
  html: string;
}

function Preview({ html }: PreviewProps) {
  return (
    <React.ViewTransition>
      <article
        className="prose"
        // oxlint-disable-next-line react-perf/jsx-no-new-object-as-prop
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </React.ViewTransition>
  );
}

interface FooterProps {
  date: string;
  isPending: boolean;
  onClick: () => void;
}

function Footer({ date, isPending, onClick }: FooterProps) {
  return (
    <div className="flex items-center justify-between">
      <div className="flex lining-nums tabular-nums">
        <span>{date[0]}</span>
        <span>{date[1]}</span>
        <span>{date[2]}</span>
        <span>{date[3]}</span>
        <span>-</span>
        <span>{date[5]}</span>
        <span>{date[6]}</span>
        <span>-</span>
        <span>{date[8]}</span>
        <span>{date[9]}</span>
        <span className="ml-1">
          {date[11]}
          {date[12]}
          {date[13]}
        </span>
      </div>
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
