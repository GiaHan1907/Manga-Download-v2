import { Star } from "lucide-react";
import { useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Input from "../../components/ui/Input";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import type { DownloadStatus } from "../../domain/download";
import type { MangaDetail } from "../../domain/manga";
import { services } from "../../services/provider";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useServicesVersion } from "../../shared/hooks/useServices";
import ChapterSelectRow from "./components/ChapterSelectRow";

type QueueMessage = { added: number; skipped: number } | null;

export default function MangaDetailPage() {
  const { mangaId = "" } = useParams();
  const version = useServicesVersion();
  const [chapterQuery, setChapterQuery] = useState("");
  const [sortAsc, setSortAsc] = useState(true);
  const [selected, setSelected] = useState<string[]>([]);
  const [queueMessage, setQueueMessage] = useState<QueueMessage>(null);
  const [queueError, setQueueError] = useState<string | null>(null);

  const detail = useAsyncData<MangaDetail | null>(() => services.catalog.getManga(mangaId), [mangaId, version]);
  const extra = useAsyncData(
    async () => {
      const [tasks, favorite] = await Promise.all([
        services.downloads.listTasks(),
        services.library.isFavorite(mangaId),
      ]);
      const statusByChapter: Record<string, DownloadStatus> = {};
      for (const task of tasks) {
        if (task.status !== "cancelled") {
          statusByChapter[task.chapterId] = task.status;
        }
      }
      return { statusByChapter, favorite };
    },
    [mangaId, version],
  );

  const visibleChapters = useMemo(() => {
    if (detail.state !== "success" || !detail.data) {
      return [];
    }
    const q = chapterQuery.trim().toLowerCase();
    const filtered = detail.data.chapters.filter((chapter) => !q || chapter.title.toLowerCase().includes(q));
    return [...filtered].sort((a, b) => {
      const an = a.number ?? Number.MAX_SAFE_INTEGER;
      const bn = b.number ?? Number.MAX_SAFE_INTEGER;
      if (an !== bn) {
        return sortAsc ? an - bn : bn - an;
      }
      return a.id.localeCompare(b.id);
    });
  }, [detail, chapterQuery, sortAsc]);

  if (detail.state === "loading" || extra.state === "loading") {
    return (
      <div className="page-stack" aria-busy="true">
        <Skeleton height="10rem" />
        <Skeleton height="20rem" />
      </div>
    );
  }

  if (detail.state === "error" || detail.data === null) {
    return (
      <ErrorState
        title="Không tìm thấy manga này"
        description={`Manga "${mangaId}" không có trong catalog demo. Có thể URL cũ từ Phase 02 hoặc id không tồn tại.`}
        action={<Button to="/discover" variant="primary">Quay lại Discover</Button>}
      />
    );
  }

  const manga = detail.data;

  async function queueSelected() {
    setQueueError(null);
    try {
      const result = await services.downloads.queueChapters(selected);
      setQueueMessage(result);
      setSelected([]);
    } catch {
      setQueueError("Thêm vào hàng đợi demo thất bại. Giữ nguyên lựa chọn để thử lại.");
    }
  }

  return (
    <div className="page-stack">
      <section className="manga-hero">
        <div className="manga-hero__cover">
          {manga.coverAsset ? <img src={manga.coverAsset} alt="" /> : null}
        </div>
        <div className="manga-hero__body">
          <div className="manga-hero__topline">
            <h2>{manga.title}</h2>
            <Button
              variant={extra.state === "success" && extra.data.favorite ? "primary" : "secondary"}
              aria-pressed={extra.state === "success" && extra.data.favorite}
              onClick={() => void services.library.toggleFavorite(manga.id)}
            >
              <Star size={16} aria-hidden="true" />
              {extra.state === "success" && extra.data.favorite ? "Đang yêu thích" : "Yêu thích"}
            </Button>
          </div>
          <p className="muted">
            {manga.author} · nguồn demo: {manga.sourceId} ({manga.sourceMangaId})
          </p>
          <div className="chip-row">
            <StatusBadge tone="neutral">{manga.status}</StatusBadge>
            {manga.genres.map((genre) => (
              <span key={genre} className="chip">
                {genre}
              </span>
            ))}
          </div>
          <p>{manga.description}</p>
        </div>
      </section>

      <section className="panel" aria-label="Danh sách chương demo">
        <div className="section-header">
          <h2 className="panel__title">Chương ({manga.chapters.length})</h2>
          <div className="chapter-toolbar">
            <Input
              label="Lọc chương"
              type="search"
              value={chapterQuery}
              onChange={(event) => setChapterQuery(event.target.value)}
            />
            <Button variant="ghost" onClick={() => setSortAsc((value) => !value)}>
              Sắp xếp: {sortAsc ? "tăng dần" : "giảm dần"}
            </Button>
            <Button variant="ghost" onClick={() => setSelected(visibleChapters.map((chapter) => chapter.id))}>
              Chọn tất cả đang hiển thị
            </Button>
            <Button variant="ghost" onClick={() => setSelected([])}>
              Bỏ chọn
            </Button>
          </div>
        </div>

        {manga.chapters.length === 0 && (
          <EmptyState title="Chưa có chương mẫu" description="Manga demo này chưa được seed chương trong fixture catalog." />
        )}

        {manga.chapters.length > 0 && visibleChapters.length === 0 && (
          <p className="muted">Không chương nào khớp bộ lọc “{chapterQuery}”.</p>
        )}

        <div className="chapter-list">
          {visibleChapters.map((chapter) => (
            <ChapterSelectRow
              key={chapter.id}
              chapter={chapter}
              selected={selected.includes(chapter.id)}
              onSelectChange={(chapterId, checked) =>
                setSelected((current) =>
                  checked ? [...new Set([...current, chapterId])] : current.filter((id) => id !== chapterId),
                )
              }
              queueStatus={extra.state === "success" ? extra.data.statusByChapter[chapter.id] : undefined}
            />
          ))}
        </div>

        <div className="chapter-toolbar__footer">
          <span className="muted" aria-live="polite">
            Đã chọn {selected.length}/{visibleChapters.length}
          </span>
          <Button variant="primary" disabled={selected.length === 0} onClick={() => void queueSelected()}>
            Add selected to demo queue
          </Button>
        </div>
        {queueMessage && (
          <p className="muted" role="status">
            Hàng đợi demo: thêm mới {queueMessage.added}, bỏ qua {queueMessage.skipped} (đã có trong hàng đợi hoặc không hợp lệ). Không có file nào được tải.
          </p>
        )}
        {queueError && (
          <p className="queue-error" role="alert">
            {queueError}
          </p>
        )}
      </section>
    </div>
  );
}
