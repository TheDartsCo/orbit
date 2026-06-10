import { formatCompactNumber, niceMaximum } from "../statisticsFormat";

interface HorizontalBarChartProps {
  title: string;
  rows: Array<{ key: string; label: string; value: number }>;
  colorForRow: (key: string, index: number) => string;
}

export function HorizontalBarChart({
  title,
  rows,
  colorForRow,
}: HorizontalBarChartProps) {
  const visibleRows = rows.slice(0, 10);
  const max = niceMaximum(Math.max(0, ...visibleRows.map((row) => row.value)));
  const width = 720;
  const rowHeight = 42;
  const margin = { top: 8, right: 62, bottom: 10, left: 130 };
  const plotWidth = width - margin.left - margin.right;
  const height = margin.top + margin.bottom + visibleRows.length * rowHeight;

  if (visibleRows.length === 0) {
    return <EmptyChart />;
  }

  return (
    <svg
      className="h-auto w-full"
      viewBox={`0 0 ${width} ${height}`}
      role="img"
      aria-label={title}
    >
      <title>{title}</title>
      {visibleRows.map((row, index) => {
        const y = margin.top + index * rowHeight;
        const barWidth = (row.value / max) * plotWidth;
        return (
          <g key={row.key}>
            <text
              x={margin.left - 12}
              y={y + 22}
              textAnchor="end"
              fill="var(--color-text-secondary)"
              fontSize="13"
            >
              {row.label}
            </text>
            <rect
              x={margin.left}
              y={y + 7}
              width={Math.max(1, barWidth)}
              height={22}
              rx={4}
              fill={colorForRow(row.key, index)}
              tabIndex={0}
            >
              <title>{`${row.label}: ${formatCompactNumber(row.value)}`}</title>
            </rect>
            <text
              x={margin.left + barWidth + 8}
              y={y + 22}
              fill="var(--color-text-muted)"
              fontSize="12"
            >
              {formatCompactNumber(row.value)}
            </text>
          </g>
        );
      })}
    </svg>
  );
}

function EmptyChart() {
  return (
    <div className="flex h-52 items-center justify-center text-sm text-text-muted">
      No data for this period
    </div>
  );
}
