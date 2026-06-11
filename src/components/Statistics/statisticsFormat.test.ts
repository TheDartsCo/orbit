import { formatBucketDate } from "./statisticsFormat.ts";

const date = "2026-06-05T00:00:00.000Z";

const cases = [
  {
    period: "7d" as const,
    expected: new Date(date).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      timeZone: "UTC",
    }),
  },
  {
    period: "30d" as const,
    expected: new Date(date).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      timeZone: "UTC",
    }),
  },
  {
    period: "90d" as const,
    expected: new Date(date).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      timeZone: "UTC",
    }),
  },
  {
    period: "all" as const,
    expected: new Date(date).toLocaleDateString(undefined, {
      month: "short",
      year: "numeric",
      timeZone: "UTC",
    }),
  },
];

for (const { period, expected } of cases) {
  const actual = formatBucketDate(date, period);
  if (actual !== expected) {
    throw new Error(`${period}: expected "${expected}", received "${actual}"`);
  }
}
