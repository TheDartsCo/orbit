import { formatCompactNumber, formatPercentage } from "../statisticsFormat";
import { ChartLegend } from "./ChartLegend";

interface DonutChartProps {
  title: string;
  rows: Array<{ key: string; label: string; value: number; percentage: number }>;
  colorForRow: (key: string, index: number) => string;
}

export function DonutChart({ title, rows, colorForRow }: DonutChartProps) {
  const total = rows.reduce((sum, row) => sum + row.value, 0);
  if (total === 0) {
    return (
      <div className="flex h-64 items-center justify-center text-sm text-text-muted">
        No token data for this period
      </div>
    );
  }

  let offset = 0;
  return (
    <div>
      <ChartLegend
        items={rows.map((row, index) => ({
          key: row.key,
          label: row.label,
          color: colorForRow(row.key, index),
          value: formatPercentage(row.percentage),
        }))}
      />
      <svg
        className="mx-auto mt-5 h-64 w-64"
        viewBox="0 0 240 240"
        role="img"
        aria-label={title}
      >
        <title>{title}</title>
        <circle cx="120" cy="120" r="82" fill="none" stroke="var(--color-bg-primary)" strokeWidth="42" />
        {rows.map((row, index) => {
          const length = (row.value / total) * 100;
          const circle = (
            <circle
              key={row.key}
              cx="120"
              cy="120"
              r="82"
              fill="none"
              stroke={colorForRow(row.key, index)}
              strokeWidth="42"
              pathLength="100"
              strokeDasharray={`${length} ${100 - length}`}
              strokeDashoffset={-offset}
              transform="rotate(-90 120 120)"
              tabIndex={0}
            >
              <title>{`${row.label}: ${formatCompactNumber(row.value)} (${formatPercentage(row.percentage)})`}</title>
            </circle>
          );
          offset += length;
          return circle;
        })}
        <text x="120" y="116" textAnchor="middle" fill="var(--color-text-muted)" fontSize="13">
          Total tokens
        </text>
        <text x="120" y="140" textAnchor="middle" fill="var(--color-text-primary)" fontSize="20" fontWeight="700">
          {formatCompactNumber(total)}
        </text>
      </svg>
    </div>
  );
}
