"use client";

import React from "react";

// Minimal ErrorBoundary example to handle errors globally on browser
export function GlobalErrorBoundary(props: { children?: React.ReactNode }) {
  return <ErrorBoundary errorComponent={DefaultGlobalErrorPage}>{props.children}</ErrorBoundary>;
}

// https://github.com/vercel/next.js/blob/33f8428f7066bf8b2ec61f025427ceb2a54c4bdf/packages/next/src/client/components/error-boundary.tsx
// https://react.dev/reference/react/Component#catching-rendering-errors-with-an-error-boundary
class ErrorBoundary extends React.Component<{
  children?: React.ReactNode;
  errorComponent: React.FC<{
    error: Error;
    reset: () => void;
  }>;
}> {
  // oxlint-disable-next-line react/state-in-constructor -- class field syntax is idiomatic modern JS
  state: { error?: Error } = {};

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  reset = () => {
    // oxlint-disable-next-line react/no-set-state -- `ErrorBoundary` requires class component
    this.setState({ error: null });
  };

  render() {
    const { error } = this.state;
    if (error) {
      return <this.props.errorComponent error={error} reset={this.reset} />;
    }
    return this.props.children;
  }
}

// https://github.com/vercel/next.js/blob/677c9b372faef680d17e9ba224743f44e1107661/packages/next/src/build/webpack/loaders/next-app-loader.ts#L73
// https://github.com/vercel/next.js/blob/677c9b372faef680d17e9ba224743f44e1107661/packages/next/src/client/components/error-boundary.tsx#L145
// https://github.com/vercel/next.js/blob/473ae4b70dd781cc8b2620c95766f827296e689a/packages/next/src/client/components/builtin/global-error.tsx
function DefaultGlobalErrorPage(props: { error: Error; reset: () => void }) {
  return (
    <html lang="en">
      <head>
        <title>Unexpected Error</title>
      </head>
      <body
        // oxlint-disable-next-line react-perf/jsx-no-new-object-as-prop -- error page rendered only on unhandled errors; performance is not a concern
        style={{
          fontFamily:
            'system-ui,"Segoe UI",Roboto,Helvetica,Arial,sans-serif,"Apple Color Emoji","Segoe UI Emoji"',
          height: "100vh",
          margin: 0,
          display: "flex",
          flexDirection: "column",
          placeContent: "center",
          placeItems: "center",
          fontSize: "16px",
          fontWeight: 400,
          lineHeight: "28px",
        }}
      >
        <div>Caught an unexpected error</div>
        <pre>
          Error:{" "}
          {import.meta.env.DEV && "message" in props.error ? props.error.message : "(Unknown)"}
        </pre>
        <button
          // oxlint-disable-next-line react-perf/jsx-no-new-function-as-prop -- closes over `props.reset`; cannot be extracted without binding
          onClick={() => {
            React.startTransition(() => {
              props.reset();
            });
          }}
        >
          Reset
        </button>
      </body>
    </html>
  );
}
