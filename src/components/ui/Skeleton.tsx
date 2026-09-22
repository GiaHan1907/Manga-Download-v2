export type SkeletonProps = {
  width?: string;
  height?: string;
  label?: string;
};

export default function Skeleton({ width = "100%", height = "1rem", label = "Đang tải" }: SkeletonProps) {
  return <div className="skeleton" style={{ width, height }} aria-label={label} />;
}
