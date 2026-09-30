/// Vietnamese relative age from an ISO timestamp, e.g. "2 giờ trước".
/// Returns "" for a missing or unparseable value so callers render nothing
/// rather than a fabricated age.
export function relativeAge(iso: string | null | undefined, now: number = Date.now()): string {
  if (!iso) {
    return "";
  }
  const at = Date.parse(iso);
  if (Number.isNaN(at)) {
    return "";
  }
  const seconds = Math.floor((now - at) / 1000);
  if (seconds < 60) {
    return seconds <= 5 ? "vừa xong" : `${seconds} giây trước`;
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return `${minutes} phút trước`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours} giờ trước`;
  }
  const days = Math.floor(hours / 24);
  if (days < 30) {
    return `${days} ngày trước`;
  }
  const months = Math.floor(days / 30);
  if (months < 12) {
    return `${months} tháng trước`;
  }
  return `${Math.floor(months / 12)} năm trước`;
}
