import { progressPercent } from "../readerLogic";

export type ReaderProgressRingProps = {
  page: number;
  pageCount: number;
};

/// Pure presentation ring for the reader corner. Reuses the tested
/// `progressPercent` math; no state, no IPC.
export default function ReaderProgressRing({ page, pageCount }: ReaderProgressRingProps) {
  const percent = progressPercent(page, pageCount);
  const radius = 15;
  const circumference = 2 * Math.PI * radius;
  const dash = (percent / 100) * circumference;
  return (
    <div className="reader-progress-ring" aria-hidden="true" title={`${percent}%`}>
      <svg width="38" height="38" viewBox="0 0 38 38">
        <circle
          cx="19"
          cy="19"
          r={radius}
          fill="none"
          stroke="var(--border)"
          strokeWidth="3"
        />
        <circle
          cx="19"
          cy="19"
          r={radius}
          fill="none"
          stroke="var(--primary)"
          strokeWidth="3"
          strokeLinecap="round"
          strokeDasharray={`${dash} ${circumference - dash}`}
          transform="rotate(-90 19 19)"
        />
      </svg>
      <span className="reader-progress-ring__label">{percent}%</span>
    </div>
  );
}
