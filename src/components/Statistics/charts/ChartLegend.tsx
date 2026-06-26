interface ChartLegendProps {
  items: Array<{ key: string; label: string; color: string; value?: string }>;
}

export function ChartLegend({ items }: ChartLegendProps) {
  return (
    <div className="flex flex-wrap gap-x-4 gap-y-2 text-xs text-text-secondary">
      {items.map((item) => (
        <span key={item.key} className="flex items-center gap-1.5">
          <span
            className="h-2.5 w-2.5 rounded-sm"
            style={{ backgroundColor: item.color }}
          />
          <span>{item.label}</span>
          {item.value ? <span className="text-text-muted">{item.value}</span> : null}
        </span>
      ))}
    </div>
  );
}
