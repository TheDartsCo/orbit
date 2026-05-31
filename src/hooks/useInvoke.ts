import { invoke } from "@tauri-apps/api/core";
import { useCallback } from "react";

export function useInvoke() {
  const call = useCallback(
    async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
      return invoke<T>(command, args);
    },
    []
  );

  return { invoke: call };
}
