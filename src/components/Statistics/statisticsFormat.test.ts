import { formatBucketDate } from "./statisticsFormat.ts";

const date = "2026-06-05T00:00:00.000Z";

const cases = [
  { period: "7d" as const, expected: "Jun 5" },
  { period: "30d" as const, expected: "Jun 5" },
  { period: "90d" as const, expected: "Jun 5" },
  { period: "all" as const, expected: "Jun 2026" },
];

for (const { period, expected } of cases) {
  const actual = formatBucketDate(date, period);
  if (actual !== expected) {
    throw new Error(`${period}: expected "${expected}", received "${actual}"`);
  }
}
