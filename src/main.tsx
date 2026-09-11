import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createRouter } from "@tanstack/react-router";
import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { Toaster } from "sonner";
import "./theme.css";
import { Palette } from "./components/Palette";
import { queryClient } from "./lib/query";
import { useTheme } from "./lib/theme";
import { routeTree } from "./routeTree.gen";

const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

function Bootstrap({ children }: { children: React.ReactNode }) {
  const init = useTheme((s) => s.init);
  const ready = useTheme((s) => s.ready);
  const [done, setDone] = useState(false);

  useEffect(() => {
    void init().finally(() => setDone(true));
  }, [init]);

  if (!done || !ready) {
    return (
      <div className="flex h-screen items-center justify-center bg-bg-secondary">
        <p className="text-sm text-text-tertiary">Memuat PeopleX…</p>
      </div>
    );
  }
  return <>{children}</>;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <Bootstrap>
        <RouterProvider router={router} />
        <Palette />
        <Toaster position="bottom-right" richColors closeButton />
      </Bootstrap>
    </QueryClientProvider>
  </React.StrictMode>,
);
