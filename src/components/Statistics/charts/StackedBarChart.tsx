import type { StatisticsTimeBucket } from "../../../types";
import {
  formatBucketDate,
  formatCompactNumber,
  niceMaximum,
} from "../statisticsFormat";
import { ChartLegend } from "./ChartLegend";

interface StackedBarChartProps {
  title: string;
  buckets: StatisticsTimeBucket[];
  colorForKey: (key: string, index: number) => string;
}

export function StackedBarChart({
  title,
  buckets,
  colorForKey,
}: StackedBarChartProps) {
  const series = collectSeries(buckets);
  const max = niceMaximum(
    Math.max(0, ...buckets.map((bucket) => bucket.values.reduce((sum, item) => sum + item.value, 0)))
  );
  const width = 1000;
  const height = 340;
  const margin = { top: 16, right: 18, bottom: 48, left: 62 };
  const plotWidth = width - margin.left - margin.right;
  const plotHeight = height - margin.top - margin.bottom;
  const step = plotWidth / Math.max(1, buckets.length);
  const barWidth = Math.max(4, Math.min(52, step * 0.7));

  return (
    <div>
      <ChartLegend
        items={series.map((item, index) => ({
          ...item,
          color: colorForKey(item.key, index),
        }))}
      />
      <svg
        className="mt-4 h-auto w-full overflow-visible"
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label={title}
      >
        <title>{title}</title>
        {[0, 1, 2, 3, 4].map((tick) => {
          const value = (max * tick) / 4;
          const y = margin.top + plotHeight - (value / max) * plotHeight;
          return (
            <g key={tick}>
              <line
                x1={margin.left}
                x2={width - margin.right}
                y1={y}
                y2={y}
                stroke="var(--color-border)"
              />
              <text
                x={margin.left - 10}
                y={y + 4}
                textAnchor="end"
                fill="var(--color-text-muted)"
                fontSize="12"
              >
                {formatCompactNumber(value)}
              </text>
            </g>
          );
        })}
        {buckets.map((bucket, bucketIndex) => {
          let accumulated = 0;
          const x = margin.left + bucketIndex * step + (step - barWidth) / 2;
          return (
            <g key={bucket.start}>
              {bucket.values.map((item) => {
                const seriesIndex = series.findIndex((seriesItem) => seriesItem.key === item.key);
                const segmentHeight = (item.value / max) * plotHeight;
                const y = margin.top + plotHeight - accumulated - segmentHeight;
                accumulated += segmentHeight;
                return (
                  <rect
                    key={item.key}
                    x={x}
                    y={y}
                    width={barWidth}
                    height={Math.max(0, segmentHeight)}
                    fill={colorForKey(item.key, seriesIndex)}
                    tabIndex={0}
                  >
                    <title>{`${item.label}: ${formatCompactNumber(item.value)}`}</title>
                  </rect>
                );
              })}
              {(buckets.length <= 14 || bucketIndex % Math.ceil(buckets.length / 10) === 0) && (
                <text
                  x={x + barWidth / 2}
                  y={height - 16}
                  textAnchor="middle"
                  fill="var(--color-text-muted)"
                  fontSize="12"
                >
                  {formatBucketDate(bucket.start, buckets.length)}
                </text>
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
}

function collectSeries(buckets: StatisticsTimeBucket[]) {
  const totals = new Map<string, { key: string; label: string; total: number }>();
  for (const bucket of buckets) {
    for (const item of bucket.values) {
      const existing = totals.get(item.key);
      totals.set(item.key, {
        key: item.key,
        label: item.label,
        total: (existing?.total ?? 0) + item.value,
      });
    }
  }
  return Array.from(totals.values()).sort((a, b) => b.total - a.total);
}
