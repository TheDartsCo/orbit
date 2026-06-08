import type { MessageRole } from "../types";

export function createDefaultEnabledRoles() {
  return new Set<MessageRole>(["user", "assistant", "tool"]);
}
