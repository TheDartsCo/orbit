import type { ReactNode } from "react";

export function StatCard({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail: string;
}) {
  return (
    <div className="rounded-xl border border-border bg-bg-secondary p-5 shadow-sm">
      <div className="text-sm font-medium text-text-secondary">{label}</div>
      <div className="mt-2 text-3xl font-bold tracking-tight text-text-primary">{value}</div>
      <div className="mt-2 text-xs font-medium text-text-muted">{detail}</div>
    </div>
  );
}

export function ChartPanel({
  title,
  children,
  className = "",
}: {
  title: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`rounded-xl border border-border bg-bg-secondary p-5 ${className}`}>
      <h2 className="mb-4 text-sm font-bold text-text-primary">{title}</h2>
      {children}
    </section>
  );
}

export function DataTable({
  title,
  headers,
  children,
}: {
  title: string;
  headers: string[];
  children: ReactNode;
}) {
  return (
    <section>
      <h2 className="mb-3 text-sm font-bold text-text-primary">{title}</h2>
      <div className="overflow-x-auto rounded-xl border border-border bg-bg-secondary">
        <table className="w-full min-w-[760px] border-collapse text-left text-sm">
          <thead>
            <tr className="border-b border-border text-xs text-text-muted">
              {headers.map((header, index) => (
                <th
                  key={header}
                  className={`px-5 py-3 font-semibold ${index > 0 ? "text-right" : ""}`}
                >
                  {header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>{children}</tbody>
        </table>
      </div>
    </section>
  );
}
