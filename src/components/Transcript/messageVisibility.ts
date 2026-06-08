import type { Message } from "../../types";

function hasText(value: string | null | undefined) {
  return Boolean(value?.trim());
}

export function isDisplayableMessage(message: Message) {
  if (hasText(message.content)) {
    return true;
  }

  if (message.role !== "tool") {
    return false;
  }

  return (
    hasText(message.tool_name) ||
    hasText(message.tool_input) ||
    hasText(message.tool_output)
  );
}
