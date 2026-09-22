import { BookOpen, Play } from "lucide-react";
import Button from "../../../components/ui/Button";
import EmptyState from "../../../components/ui/EmptyState";
import Progress from "../../../components/ui/Progress";
import type { ReadingProgress } from "../../../domain/reader";

export type ContinueReadingProps = {
  progress: (ReadingProgress & { totalPages: number }) | null;
  title: string | null;
};

export default function ContinueReading({ progress, title }: ContinueReadingProps) {
  if (!progress || !title || progress.totalPages === 0) {
    return (
      <EmptyState
        title="Chưa có lịch sử đọc mẫu"
        description="Mở thử trang Library để khám phá dữ liệu mẫu của bản prototype."
        action={<Button to="/library" variant="secondary">Tới Library</Button>}
      />
    );
  }

  const page = Math.min(progress.page + 1, progress.totalPages);

  return (
    <section className="continue-reading">
      <div className="continue-reading__cover">
        <BookOpen size={28} aria-hidden="true" />
      </div>
      <div className="continue-reading__body">
        <p className="muted">Tiếp tục đọc</p>
        <h3>{title}</h3>
        <p className="muted">
          Trang {page}/{progress.totalPages} · Demo progress
        </p>
        <Progress
          value={Math.round((page / progress.totalPages) * 100)}
          label={`Tiến độ demo: trang ${page}/${progress.totalPages}`}
        />
      </div>
      <Button to={`/reader/${progress.mangaId}/${progress.chapterId}`} variant="primary">
        <Play size={16} aria-hidden="true" />
        Tiếp tục
      </Button>
    </section>
  );
}
