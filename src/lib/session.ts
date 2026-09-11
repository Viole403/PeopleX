import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";
import { toast } from "sonner";
import { commands, type SessionUser } from "../bindings";
import { unwrap } from "./query";

export function useSession() {
  return useQuery({
    queryKey: ["session"],
    queryFn: () => unwrap(commands.sessionState()),
    staleTime: 60_000,
  });
}

export function can(session: SessionUser | null | undefined, ...perms: string[]): boolean {
  if (!session) return false;
  if (session.is_super_admin) return true;
  return perms.some((p) => session.permissions.includes(p));
}

export function useLogin() {
  const queryClient = useQueryClient();
  const router = useRouter();
  return useMutation({
    mutationFn: (v: { username: string; password: string }) =>
      unwrap(commands.login(v.username, v.password)),
    onSuccess: (data) => {
      queryClient.setQueryData(["session"], data.user);
      toast.success(`Selamat datang, ${data.user.username}`);
      void router.navigate({
        to: data.must_change_password ? "/change-password" : "/",
      });
    },
    onError: (e: Error) => toast.error(e.message),
  });
}

export function useLogout() {
  const queryClient = useQueryClient();
  const router = useRouter();
  return useMutation({
    mutationFn: () => unwrap(commands.logout()),
    onSuccess: () => {
      queryClient.setQueryData(["session"], null);
      void queryClient.invalidateQueries();
      void router.navigate({ to: "/login" });
    },
    onError: (e: Error) => toast.error(e.message),
  });
}
