import type { StatisticsPeriod } from "../../types";

export function formatCompactNumber(value: number): string {
  const absolute = Math.abs(value);
  if (absolute >= 1_000_000_000) return formatUnit(value, 1_000_000_000, "B");
  if (absolute >= 1_000_000) return formatUnit(value, 1_000_000, "M");
  if (absolute >= 1_000) return formatUnit(value, 1_000, "k");
  return value.toLocaleString();
}

export function formatPercentage(value: number): string {
  return `${Math.round(value)}%`;
}

export function formatRelativeTime(value: string): string {
  const timestamp = new Date(value).getTime();
  if (!Number.isFinite(timestamp)) return "-";
  const minutes = Math.max(0, Math.floor((Date.now() - timestamp) / 60_000));
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(value).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function formatBucketDate(
  value: string,
  period: StatisticsPeriod
): string {
  const date = new Date(value);
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: period === "all" ? undefined : "numeric",
    year: period === "all" ? "numeric" : undefined,
    timeZone: "UTC",
  });
}

export function niceMaximum(value: number): number {
  if (value <= 0) return 1;
  const magnitude = 10 ** Math.floor(Math.log10(value));
  const normalized = value / magnitude;
  const nice = normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10;
  return nice * magnitude;
}

function formatUnit(value: number, divisor: number, suffix: string): string {
  const scaled = value / divisor;
  const digits = Math.abs(scaled) >= 100 || Number.isInteger(scaled) ? 0 : 1;
  return `${scaled.toFixed(digits).replace(/\.0$/, "")}${suffix}`;
}
