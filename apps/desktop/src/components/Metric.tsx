interface MetricProps {
  readonly value: string | number;
  readonly label: string;
  readonly detail?: string;
}

export function Metric({ value, label, detail }: MetricProps) {
  return (
    <div className="metric">
      <div className="metric__value">{value}</div>
      <div className="metric__label">{label}</div>
      {detail ? <div className="metric__detail">{detail}</div> : null}
    </div>
  );
}
