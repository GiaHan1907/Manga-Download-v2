export type ProgressProps = {
  value: number;
  label: string;
};

export default function Progress({ value, label }: ProgressProps) {
  const safeValue = Math.min(100, Math.max(0, value));

  return (
    <div className="progress" role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={safeValue}>
      <div className="progress__bar" style={{ width: `${safeValue}%` }} />
    </div>
  );
}
