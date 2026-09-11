import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

/** Buka hasil command specta ({status, data/error}) atau lempar Error. */
export async function unwrap<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: unknown }>): Promise<T> {
  const r = await p;
  if (r.status === "error") {
    throw new Error(typeof r.error === "string" ? r.error : "Perintah gagal");
  }
  return r.data;
}
