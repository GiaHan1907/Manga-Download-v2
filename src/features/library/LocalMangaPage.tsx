import { Download, FolderOpen, Play, Star, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import type { LocalMangaDetailDto } from "../../types/backend";
import type { SourceViewDto } from "../../types/source";
import type { DownloadedChapterDto } from "../../types/library";
import type { QueueResultDto } from "../../types/downloads";
import { localServices } from "../../services/provider";
import { formatBytes } from "../../types/downloads";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";

const MAX_BATCH = 100;

export default function LocalMangaPage() {
  const { mangaId = "" } = useParams();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<QueueResultDto | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [favoriteBusy, setFavoriteBusy] = useState(false);
  const [nonce, setNonce] = useState(0);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [confirmWipe, setConfirmWipe] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const libraryVersion = useDomainVersion("library");
  const downloadsVersion = useDomainVersion("downloads");

  const manga = useAsyncData<LocalMangaDetailDto | null>(
    () => localServices.library.getManga(mangaId),
    [mangaId, nonce, libraryVersion],
  );
  const sources = useAsyncData<SourceViewDto[]>(() => localServices.source.listSources(), []);
  const downloaded = useAsyncData<DownloadedChapterDto[]>(
    () => localServices.libraryV2.downloadedChapters(mangaId),
    [mangaId, nonce, libraryVersion, downloadsVersion],
  );
  const progress = useAsyncData(
    () => localServices.reader.getProgress(mangaId).catch(() => null),
    [mangaId, nonce, libraryVersion],
  );

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
  const downloadedIds = new Set(
    downloaded.state === "success" ? downloaded.data.map((chapter) => chapter.id) : [],
  );
  const progressChapter =
    progress.state === "success" && progress.data && downloadedIds.has(progress.data.chapterId)
      ? progress.data
      : null;
  const actionable = detail.chapters.filter((chapter) => !downloadedIds.has(chapter.id));

  function toggle(chapterId: string, on: boolean) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (on) {
        next.add(chapterId);
      } else {
        next.delete(chapterId);
      }
      return next;
    });
  }
  function toggleAll(on: boolean) {
    setSelected(
      on ? new Set(actionable.slice(0, MAX_BATCH).map((chapter) => chapter.id)) : new Set(),
    );
  }

  async function submit() {
    if (selected.size === 0) return;
    setSubmitting(true);
    setSubmitError(null);
    setResult(null);
    try {
      const outcome = await localServices.downloads.queue({ chapterIds: [...selected] });
      setResult(outcome);
      setSelected(new Set());
      bumpDomain("downloads");
      bumpDomain("home");
      setNonce((value) => value + 1);
    } catch (error) {
      setSubmitError(error instanceof Error ? error.message : "Không xếp hàng được.");
    } finally {
      setSubmitting(false);
    }
  }

  async function toggleFavorite() {
    setFavoriteBusy(true);
    setActionError(null);
    try {
      await localServices.library.setFavorite(detail.id, !detail.favorite);
      bumpDomain("favorites");
      bumpDomain("library");
      bumpDomain("home");
    } catch (error) {
      setActionError(
        `Không cập nhật được yêu thích: ${error instanceof Error ? error.message : "lỗi IPC"}`,
      );
    } finally {
      setFavoriteBusy(false);
    }
  }

  async function removeChapter(chapterId: string) {
    setActionError(null);
    setConfirmDelete(null);
    try {
      await localServices.libraryV2.removeChapter(chapterId);
      bumpDomain("library");
      bumpDomain("home");
      setNonce((value) => value + 1);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Không xóa được.");
    }
  }

  async function removeAll() {
    setActionError(null);
    setConfirmWipe(false);
    try {
      const outcome = await localServices.libraryV2.removeMangaFiles(detail.id);
      if (outcome.failed.length > 0) {
        setActionError(`Đã xóa ${outcome.removed.length} chương, ${outcome.failed.length} chương lỗi (giữ nguyên trạng thái cũ).`);
      }
      bumpDomain("library");
      bumpDomain("home");
      setNonce((value) => value + 1);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Không xóa được.");
    }
  }

  return (
    <div className="page-stack">
      <section className="panel" aria-label="Thông tin truyện đã lưu">
        <div className="section-header">
          <div>
            <p className="muted">
              Library · Local (SQLite){" "}
              {canDownload ? (
                <StatusBadge tone="success">Nguồn cho phép tải</StatusBadge>
              ) : (
                <StatusBadge tone="neutral">Nguồn chỉ đọc metadata</StatusBadge>
              )}
            </p>
            <h1>{detail.title}</h1>
            <p className="muted">
              {detail.author ?? "Không rõ tác giả"} · {detail.chapterCount} chương ·{" "}
              {downloadedIds.size} đã tải
            </p>
          </div>
          <div className="row-actions">
            {progressChapter && (
              <Button to={`/read/${progressChapter.chapterId}`} variant="primary">
                <Play size={16} aria-hidden="true" />
                Đọc tiếp
              </Button>
            )}
            <Button variant="secondary" onClick={toggleFavorite} disabled={favoriteBusy}>
              <Star size={16} aria-hidden="true" />
              {detail.favorite ? "Bỏ yêu thích" : "Yêu thích"}
            </Button>
          </div>
        </div>
      </section>

      <section className="panel" aria-label="Chương đã tải offline">
        <h2 className="panel__title">Đã tải ({downloadedIds.size})</h2>
        {actionError && <p className="queue-error" role="alert">{actionError}</p>}
        {downloaded.state === "loading" && <Skeleton height="6rem" />}
        {downloaded.state === "error" && (
          <p className="queue-error">Không đọc được danh sách file đã tải.</p>
        )}
        {downloaded.state === "success" && downloaded.data.length === 0 && (
          <p className="muted">Chưa có chương nào tải xong. Chọn bên dưới để xếp hàng.</p>
        )}
        {downloaded.state === "success" && downloaded.data.length > 0 && (
          <>
            <ul className="chapter-select-list">
              {downloaded.data.map((chapter) => (
                <li key={chapter.id} className="chapter-select-row">
                  <span className="chapter-select-row__text">
                    <span className="chapter-row__title">{chapter.title}</span>
                    <span className="chapter-row__meta">
                      {chapter.pageCount !== null ? `${chapter.pageCount} trang` : "Không rõ số trang"}
                      {chapter.sizeBytes ? ` · ${formatBytes(chapter.sizeBytes)}` : ""}
                      {" · "}
                      {chapter.storageFormat === "folder" ? "thư mục ảnh" : "CBZ"}
                    </span>
                  </span>
                  {chapter.artifactOk ? (
                    <>
                      <Button to={`/read/${chapter.id}`} variant="secondary">
                        <Play size={14} aria-hidden="true" />
                        Đọc
                      </Button>
                      {confirmDelete === chapter.id ? (
                        <span className="row-actions" role="group" aria-label="Xác nhận xóa">
                          <Button
                            variant="primary"
                            onClick={() => void removeChapter(chapter.id)}
                          >
                            Xác nhận xóa file
                          </Button>
                          <Button variant="ghost" onClick={() => setConfirmDelete(null)}>
                            Hủy
                          </Button>
                        </span>
                      ) : (
                        <Button
                          variant="ghost"
                          onClick={() => setConfirmDelete(chapter.id)}
                          aria-label={`Xóa file chương ${chapter.title}`}
                        >
                          <Trash2 size={14} aria-hidden="true" />
                          Xóa file
                        </Button>
                      )}
                    </>
                  ) : (
                    <StatusBadge tone="error">File missing — cần tải lại</StatusBadge>
                  )}
                </li>
              ))}
            </ul>
            {confirmWipe ? (
              <p className="row-actions" role="group" aria-label="Xác nhận xóa toàn bộ">
                <Button variant="primary" onClick={() => void removeAll()}>
                  Xác nhận xóa {downloadedIds.size} file đã tải
                </Button>
                <Button variant="ghost" onClick={() => setConfirmWipe(false)}>
                  Hủy
                </Button>
              </p>
            ) : (
              <Button variant="ghost" onClick={() => setConfirmWipe(true)}>
                <FolderOpen size={14} aria-hidden="true" />
                Xóa mọi file của truyện này
              </Button>
            )}
            <p className="muted">
              Metadata, favorites và tiến độ đọc được giữ nguyên; chỉ file offline bị xóa.
            </p>
          </>
        )}
      </section>

      <section className="panel" aria-label="Tải chương">
        <div className="section-header section-header--compact">
          <h2 className="panel__title">Tải chương</h2>
          <div className="chip-row">
            <Button variant="ghost" onClick={() => toggleAll(true)} disabled={!canDownload}>
              Chọn tất cả
            </Button>
            <Button variant="ghost" onClick={() => setSelected(new Set())} disabled={selected.size === 0}>
              Bỏ chọn
            </Button>
            <Button
              variant="primary"
              onClick={submit}
              disabled={!canDownload || submitting || selected.size === 0}
            >
              <Download size={16} aria-hidden="true" />
              {submitting ? "Đang xếp hàng…" : `Tải ${selected.size} chương`}
            </Button>
          </div>
        </div>
        {!canDownload && (
          <p className="panel__description">
            Nguồn này chưa khai báo khả năng tải trang (hoặc là MangaDex — cần at-home
            host động chưa được duyệt). Nút tải bị ẩn và server từ chối mọi yêu cầu tải
            cho truyện này.
          </p>
        )}
        {submitError && <p className="queue-error" role="alert">{submitError}</p>}
        {result && (
          <div className="queue-result" role="status">
            <p>
              Đã tạo {result.tasks.length} tác vụ · tái sử dụng {result.reused.length} · lỗi{" "}
              {result.errors.length}.
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
            <Button to="/downloads" variant="secondary">
              Mở Downloads
            </Button>
          </div>
        )}
        {actionable.length === 0 ? (
          <EmptyState
            title="Mọi chương đã có offline"
            description="Không còn chương nào chưa tải của truyện này."
          />
        ) : (
          <ul className="chapter-select-list">
            {actionable.map((chapter) => (
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
                    {chapter.downloaded ? " · đã tải" : ""}
                  </span>
                </label>
              </li>
            ))}
          </ul>
        )}
        <p className="muted">Định dạng xuất theo Settings → Output (mặc định CBZ).</p>
      </section>
    </div>
  );
}
