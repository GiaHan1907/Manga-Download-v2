import { Play } from "lucide-react";
import Button from "../../../components/ui/Button";
import StatusBadge, { type StatusBadgeTone } from "../../../components/ui/StatusBadge";
import Tooltip from "../../../components/ui/Tooltip";
import type { ChapterSummary } from "../../../domain/manga";
import type { DownloadStatus } from "../../../domain/download";

export type ChapterSelectRowProps = {
  chapter: ChapterSummary;
  selected: boolean;
  onSelectChange: (chapterId: string, selected: boolean) => void;
  queueStatus?: DownloadStatus;
};

const statusTone: Record<DownloadStatus, StatusBadgeTone> = {
  completed: "success",
  queued: "warning",
  paused: "neutral",
  failed: "error",
  cancelled: "neutral",
};

export default function ChapterSelectRow({ chapter, selected, onSelectChange, queueStatus }: ChapterSelectRowProps) {
  const checkboxId = `select-${chapter.id}`;
  return (
    <div className="chapter-select-row">
      <input
        id={checkboxId}
        type="checkbox"
        checked={selected}
        onChange={(event) => onSelectChange(chapter.id, event.target.checked)}
        aria-label={`Chọn ${chapter.title}`}
      />
      <label className="chapter-select-row__text" htmlFor={checkboxId}>
        <span className="chapter-row__title">{chapter.title}</span>
        <span className="chapter-row__meta">
          {chapter.hasSamplePages ? `${chapter.pageCount ?? 0} trang mẫu` : "Chưa có trang mẫu trong demo"}
        </span>
      </label>
      <div className="chapter-select-row__actions">
        {queueStatus && <StatusBadge tone={statusTone[queueStatus]}>{`demo: ${queueStatus}`}</StatusBadge>}
        {chapter.hasSamplePages ? (
          <Button to={`/reader/${chapter.mangaId}/${chapter.id}`} variant="secondary">
            <Play size={14} aria-hidden="true" />
            Read sample
          </Button>
        ) : (
          <Tooltip
            content="Chương này chưa có trang mẫu trong demo (dev-only) — truyện thật đã tải thì đọc ở Reader offline."
            focusableWhenDisabled
          >
            <Button disabled aria-label={`Read sample không khả dụng cho ${chapter.title}`}>
              <Play size={14} aria-hidden="true" />
              Read sample
            </Button>
          </Tooltip>
        )}
      </div>
    </div>
  );
}
