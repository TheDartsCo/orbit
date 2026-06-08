import { isDisplayableMessage } from "./messageVisibility.ts";
import type { Message } from "../../types/index.ts";

const baseMessage: Message = {
  id: "msg-1",
  session_id: "session-1",
  role: "assistant",
  content: "",
  timestamp: null,
  sequence: 0,
  tool_name: null,
  tool_input: null,
  tool_output: null,
};

const cases: Array<{ name: string; message: Message; expected: boolean }> = [
  {
    name: "hides empty assistant content",
    message: { ...baseMessage, role: "assistant", content: "   " },
    expected: false,
  },
  {
    name: "hides empty system content",
    message: { ...baseMessage, role: "system", content: "\n\t" },
    expected: false,
  },
  {
    name: "shows text content",
    message: { ...baseMessage, content: "Done" },
    expected: true,
  },
  {
    name: "shows tool messages with details",
    message: { ...baseMessage, role: "tool", tool_name: "Read" },
    expected: true,
  },
  {
    name: "hides empty tool messages",
    message: { ...baseMessage, role: "tool" },
    expected: false,
  },
];

for (const { name, message, expected } of cases) {
  if (isDisplayableMessage(message) !== expected) {
    throw new Error(`${name}: expected ${expected}`);
  }
}
