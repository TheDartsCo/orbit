import { createDefaultEnabledRoles } from "./messageRoles.ts";
import type { MessageRole } from "../types/index.ts";

const defaults = createDefaultEnabledRoles();
const expectedEnabled: MessageRole[] = ["user", "assistant", "tool"];

for (const role of expectedEnabled) {
  if (!defaults.has(role)) {
    throw new Error(`expected ${role} to be enabled by default`);
  }
}

if (defaults.has("system")) {
  throw new Error("expected system messages to be hidden by default");
}
