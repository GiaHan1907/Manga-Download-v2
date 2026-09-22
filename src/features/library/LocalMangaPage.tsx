import { Download, Star } from "lucide-react";
import { useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import type { LocalChapterDto, LocalMangaDetailDto } from "../../types/backend";
import type { SourceViewDto } from "../../types/source";
import type { QueueResultDto } from "../../types/downloads";
import { localServices } from "../../services/provider";
import { useAsyncData } from "../../shared/hooks/useAsyncData";

const MAX_BATCH = 100;

export default function LocalMangaPage() {
  const { mangaId = "" } = useParams();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<QueueResultDto | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [favoriteBusy, setFavoriteBusy] = useState(false);
  const [nonce, setNonce] = useState(0);

  const manga = useAsyncData<LocalMangaDetailDto | null>(
    () => localServices.library.getManga(mangaId),
    [mangaId, nonce],
  );
  const sources = useAsyncData<SourceViewDto[]>(() => localServices.source.listSources(), []);

  const source = useMemo(() => {
    if (manga.state !== "success" || !manga.data || sources.state !== "success") return null;
    return sources.data.find((item) => item.id === manga.data?.sourceId) ?? null;
  }, [manga, sources]);
  const canDownload = source?.capabilities.includes("pages") ?? false;

  if (manga.state === "loading" || sources.state === "loading") {
    return (
      <div className="page-stack" aria-busy="true">
        <Skeleton height="8rem" />
        <Skeleton height="22rem" />
      </div>
    );
  }
  if (manga.state === "error" || !manga.data) {
    return (
      <ErrorState
        title="Không mở được truyện đã lưu"
        description={
          manga.state === "error"
            ? manga.error instanceof Error
              ? manga.error.message
              : "Lỗi khi đọc thư viện local."
            : `Không tìm thấy manga ${mangaId} trong thư viện local.`
        }
        action={<Button to="/library" variant="primary">Về Library</Button>}
      />
    );
  }
  const detail = manga.data;

  function toggle(chapterId: string, on: boolean) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (on) next.add(chapterId);
      else next.delete(chapterId);
      return next;
    });
  }
  function toggleAll(chapters: LocalChapterDto[], on: boolean) {
    setSelected(on ? new Set(chapters.slice(0, MAX_BATCH).map((chapter) => chapter.id)) : new Set());
  }

  async function submit() {
    if (selected.size === 0) return;
    setSubmitting(true);
    setSubmitError(null);
    setResult(null);
    try {
      const outcome = await localServices.downloads.queue({
        chapterIds: [...selected],
        format: undefined,
      });
      setResult(outcome);
      setSelected(new Set());
    } catch (error) {
      setSubmitError(error instanceof Error ? error.message : "Không xếp hàng được.");
    } finally {
      setSubmitting(false);
    }
  }

  async function toggleFavorite() {
    setFavoriteBusy(true);
    try {
      await localServices.library.setFavorite(detail.id, !detail.favorite);
      setNonce((value) => value + 1);
    } finally {
      setFavoriteBusy(false);
    }
  }

  return (
    <div className="page-stack">
      <section className="panel" aria-label="Thông tin truyện đã lưu">
        <div className="section-header">
          <div>
            <p className="muted">
              Library · Local (SQLite) {canDownload ? <StatusBadge tone="success">Nguồn cho phép tải</StatusBadge> : <StatusBadge tone="neutral">Nguồn chỉ đọc metadata</StatusBadge>}
            </p>
            <h1>{detail.title}</h1>
            <p className="muted">{detail.author ?? "Không rõ tác giả"} · {detail.chapterCount} chương</p>
          </div>
          <Button variant="secondary" onClick={toggleFavorite} disabled={favoriteBusy}>
            <Star size={16} aria-hidden="true" />
            {detail.favorite ? "Bỏ yêu thích" : "Yêu thích"}
          </Button>
        </div>
        {!canDownload && (
          <p className="panel__description">
            Adapter của truyện này chưa khai báo khả năng tải trang (hoặc nguồn chỉ cho tìm kiếm). Nút tải bị ẩn —
            server vẫn từ chối mọi yêu cầu tải cho truyện này.
          </p>
        )}
      </section>

      {detail.chapters.length === 0 ? (
        <EmptyState
          title="Chưa có chương nào trong catalog"
          description="Mở lại từ Discover → Save để cập nhật danh sách chương từ nguồn."
          action={<Button to="/discover" variant="primary">Tới Discover</Button>}
        />
      ) : (
        <section className="panel">
          <div className="chip-row">
            <Button variant="ghost" onClick={() => toggleAll(detail.chapters, true)} disabled={!canDownload}>
              Chọn tất cả
            </Button>
            <Button variant="ghost" onClick={() => setSelected(new Set())} disabled={selected.size === 0}>
              Bỏ chọn
            </Button>
            <Button variant="primary" onClick={submit} disabled={!canDownload || submitting || selected.size === 0}>
              <Download size={16} aria-hidden="true" />
              {submitting ? "Đang xếp hàng…" : `Tải ${selected.size} chương`}
            </Button>
            <span className="muted">Định dạng theo Settings → Output (mặc định CBZ).</span>
          </div>
          {submitError && <p className="queue-error" role="alert">{submitError}</p>}
          {result && (
            <div className="queue-result" role="status">
              <p>
                Đã tạo {result.tasks.length} tác vụ · tái sử dụng {result.reused.length} · lỗi {result.errors.length}.
              </p>
              {result.errors.length > 0 && (
                <ul>
                  {result.errors.map((item) => (
                    <li key={`${item.chapterId}-${item.code}`}>
                      [{item.code}] {item.message}
                    </li>
                  ))}
                </ul>
              )}
              <Button to="/downloads" variant="secondary">Mở Downloads</Button>
            </div>
          )}
          <ul className="chapter-select-list">
            {detail.chapters.map((chapter) => (
              <li key={chapter.id} className="chapter-select-row">
                <input
                  id={`chapter-${chapter.id}`}
                  type="checkbox"
                  disabled={!canDownload}
                  checked={selected.has(chapter.id)}
                  onChange={(event) => toggle(chapter.id, event.target.checked)}
                  aria-label={`Chọn ${chapter.title}`}
                />
                <label htmlFor={`chapter-${chapter.id}`} className="chapter-select-row__text">
                  <span className="chapter-row__title">{chapter.title}</span>
                  <span className="chapter-row__meta">
                    {chapter.number !== null ? `Số ${chapter.number}` : "Không số"}
                    {chapter.pageCount !== null ? ` · ${chapter.pageCount} trang` : ""}
                  </span>
                </label>
                {chapter.downloaded && (
                  <StatusBadge tone="success">{`Đã tải offline (${chapter.outputFormat ?? ""})`}</StatusBadge>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
