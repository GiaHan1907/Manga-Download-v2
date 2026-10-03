import { Download, FolderInput, FolderOpen, Play, Star, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import Button from "../../components/ui/Button";
import MangaDeleteDialog from "./MangaDeleteDialog";
import Input from "../../components/ui/Input";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import { pushToast } from "../../components/ui/toastStore";
import {
  listReadHistory as desktopListReadHistory,
  clearReadHistory as desktopClearReadHistory,
} from "../../shared/api/desktop";
import type { LocalMangaDetailDto } from "../../types/backend";
import type { SourceViewDto } from "../../types/source";
import type { DownloadedChapterDto } from "../../types/library";
import type { QueueResultDto } from "../../types/downloads";
import { localServices } from "../../services/provider";
import { formatBytes } from "../../types/downloads";
import { matchesQuery } from "../../shared/lib/text";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";

const CHAPTER_PAGE = 100;

export default function LocalMangaPage() {
  const { mangaId = "" } = useParams();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [submitting, setSubmitting] = useState(false);
  const [cancelAllBusy, setCancelAllBusy] = useState(false);
  const [result, setResult] = useState<QueueResultDto | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [favoriteBusy, setFavoriteBusy] = useState(false);
  const [nonce, setNonce] = useState(0);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [confirmWipe, setConfirmWipe] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [alertsNonce, setAlertsNonce] = useState(0);
  const [importBusy, setImportBusy] = useState(false);
  const [downloadedQuery, setDownloadedQuery] = useState("");
  const [queueQuery, setQueueQuery] = useState("");
  const [chapterLimit, setChapterLimit] = useState(CHAPTER_PAGE);
  const [dropActive, setDropActive] = useState(false);
  const [eraseOpen, setEraseOpen] = useState(false);
  const navigate = useNavigate();
  const importBusyRef = useRef(false);

  const importCbzPath = async (path: string) => {
    if (importBusyRef.current) {
      return;
    }
    importBusyRef.current = true;
    setImportBusy(true);
    setActionError(null);
    try {
      const result = await localServices.libraryV2.importCbzPath({ mangaId, path });
      pushToast(`Đã nhập “${result.title}” · ${result.pageCount} trang.`);
      bumpDomain("library");
      bumpDomain("home");
      setNonce((value) => value + 1);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Nhập CBZ thất bại.");
    } finally {
      importBusyRef.current = false;
      setImportBusy(false);
    }
  };

  // Drag-and-drop import. Guarded so jsdom tests (no Tauri host) skip it.
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) {
      return;
    }
    let alive = true;
    let unlisten: (() => void) | null = null;
    void import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
      getCurrentWebview().onDragDropEvent((event) => {
        if (!alive) return;
        if (event.payload.type === "enter") {
          setDropActive(event.payload.paths.some((p) => p.toLowerCase().endsWith(".cbz")));
        } else if (event.payload.type === "leave") {
          setDropActive(false);
        } else if (event.payload.type === "drop") {
          setDropActive(false);
          const file = event.payload.paths.find((p) => p.toLowerCase().endsWith(".cbz"));
          if (file) void importCbzPath(file);
        }
      }),
    ).then((fn) => {
      if (alive) unlisten = fn;
      else fn();
    });
    return () => {
      alive = false;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mangaId]);


  const libraryVersion = useDomainVersion("library");
  const downloadsVersion = useDomainVersion("downloads");

  // Feature plan F1: watcher state + unacknowledged new-chapter alerts.
  const watch = useAsyncData(
    async () => ({
      state: await localServices.libraryV2.getWatch(mangaId),
      alerts: await localServices.libraryV2.listAlerts(mangaId),
    }),
    [mangaId, alertsNonce, libraryVersion],
  );

  // Feature plan F4: per-chapter read history for this manga (newest first).
  const history = useAsyncData(
    () => desktopListReadHistory({ mangaId, limit: 20, offset: 0 }),
    [mangaId, alertsNonce, nonce],
  );

  const manga = useAsyncData<LocalMangaDetailDto | null>(
    () => localServices.library.getManga(mangaId),
    [mangaId, nonce, libraryVersion],
  );

  // Saved-manga cover (schema stores the path key; bytes come from the same
  // get_manga_cover IPC). Null inputs before load make the hook a no-op.
  const coverUrl = useCoverUrl(
    manga.state === "success" && manga.data ? manga.data.sourceId : "",
    manga.state === "success" && manga.data ? manga.data.sourceMangaId : "",
    manga.state === "success" && manga.data ? manga.data.coverKey : null,
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
  const visibleActionable = queueQuery.trim()
    ? actionable.filter((chapter) => matchesQuery(`${chapter.title} ${chapter.number ?? ""}`, queueQuery))
    : actionable;
  const shownActionable = visibleActionable.slice(0, chapterLimit);
  const visibleDownloaded =
    downloaded.state === "success"
      ? downloaded.data.filter((chapter) =>
          matchesQuery(`${chapter.title} ${chapter.number ?? ""}`, downloadedQuery),
        )
      : [];

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
    // queueMany chunking removed the old 100-per-call cap on batch size.
    setSelected(on ? new Set(actionable.map((chapter) => chapter.id)) : new Set());
  }

  // Shared queue path for the manual selection AND the quick 5/10/all
  // buttons — downloadsApi.queueMany chunks past the 100-per-call Rust cap.
  async function queueChapters(chapterIds: string[], label: string) {
    if (chapterIds.length === 0) {
      pushToast("Không còn chương nào chưa tải để xếp hàng.");
      return;
    }
    setSubmitting(true);
    setSubmitError(null);
    setResult(null);
    try {
      const outcome = await localServices.downloads.queueMany({ chapterIds });
      setResult(outcome);
      setSelected(new Set());
      bumpDomain("downloads");
      bumpDomain("home");
      if (outcome.tasks.length > 0 || outcome.reused.length > 0) {
        pushToast(
          outcome.reused.length > 0 && outcome.tasks.length === 0
            ? `${outcome.reused.length} chương đã có sẵn trong hàng đợi (${label}).`
            : `Đã xếp hàng ${outcome.tasks.length} chương${outcome.reused.length > 0 ? ` (${outcome.reused.length} trùng đã có)` : ""} — ${label}.`,
        );
      } else if (outcome.errors.length > 0) {
        pushToast(`Không xếp hàng được: ${outcome.errors[0].message}`);
      }
      setNonce((value) => value + 1);
    } catch (error) {
      setSubmitError(error instanceof Error ? error.message : "Không xếp hàng được.");
    } finally {
      setSubmitting(false);
    }
  }

  function submit() {
    if (selected.size === 0) return;
    void queueChapters([...selected], "theo chọn");
  }

  function quickDownload(count: number | null) {
    const targets = (count === null ? actionable : actionable.slice(0, count)).map(
      (chapter) => chapter.id,
    );
    void queueChapters(targets, count === null ? "toàn bộ chương chưa tải" : `${count} chương tiếp`);
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

  function eraseOutcomeToast(outcome: { chaptersRemoved: number; filesRemoved: number }) {
    pushToast(
      `Đã gỡ truyện khỏi Thư viện (${outcome.chaptersRemoved} chương)` +
        (outcome.filesRemoved > 0 ? ` · xóa ${outcome.filesRemoved} file.` : " · file giữ nguyên."),
    );
    bumpDomain("library");
    bumpDomain("home");
    bumpDomain("downloads");
    navigate("/library");
  }

  // Feature plan F3b — the native picker + validation run entirely in Rust;
  // this only sends the manga id and reads back path-free metadata.
  async function importCbz() {
    setImportBusy(true);
    setActionError(null);
    try {
      const result = await localServices.libraryV2.importCbz({ mangaId });
      if (result === null) {
        pushToast("Đã hủy nhập CBZ.");
      } else {
        pushToast(`Đã nhập “${result.title}” · ${result.pageCount} trang.`);
        bumpDomain("library");
        bumpDomain("home");
        setNonce((value) => value + 1);
      }
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Nhập CBZ thất bại.");
    } finally {
      setImportBusy(false);
    }
  }

  return (
    <div className="page-stack">
      <section className="panel" aria-label="Thông tin truyện đã lưu">
        <div className="section-header">
          {coverUrl && (
            <img
              src={coverUrl}
              alt={`Ảnh bìa ${detail.title}`}
              width={110}
              className="local-manga__cover"
            />
          )}
          <div>
            <p className="muted">

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
            <Button variant="ghost" onClick={importCbz} disabled={importBusy}>
              <FolderInput size={16} aria-hidden="true" />
              {importBusy ? "Đang nhập…" : "Nhập CBZ"}
            </Button>
          </div>
        </div>
      </section>

      {watch.state === "success" && (
        <section className="panel" aria-label="Theo dõi chương mới">
          <div className="chapter-toolbar">
            <div>
              <h2 className="panel__title">Theo dõi chương mới</h2>
              <p className="muted">
                {watch.data.state.enabled === null
                  ? "Đang theo cài đặt chung (kiểm tra ~6 giờ/lần khi app chạy)."
                  : watch.data.state.enabled
                    ? "Đang bật cho truyện này."
                    : "Đang tắt cho truyện này."}
                {watch.data.state.failCount > 0
                  ? ` · ${watch.data.state.failCount} lần lỗi gần nhất — nguồn đang tạm lùi lịch kiểm tra.`
                  : ""}
              </p>
            </div>
            {/* Security-review follow-up: tri-state control — the old binary
                toggle could never restore "follow the global setting" (NULL). */}
            <div className="settings-actions" role="group" aria-label="Chế độ theo dõi chương mới">
              {(
                [
                  { value: true, label: "Bật theo dõi" },
                  { value: false, label: "Tắt theo dõi" },
                  { value: null, label: "Theo cài đặt chung" },
                ] as const
              ).map((option) => (
                <Button
                  key={option.label}
                  variant={watch.data.state.enabled === option.value ? "primary" : "secondary"}
                  disabled={watch.data.state.enabled === option.value}
                  onClick={() => {
                    void localServices.libraryV2
                      .setWatch({ mangaId, enabled: option.value })
                      .then(() => {
                        pushToast(
                          option.value === null
                            ? "Đã đặt theo cài đặt chung."
                            : option.value
                              ? "Đã bật theo dõi chương mới cho truyện này."
                              : "Đã tắt theo dõi chương mới cho truyện này.",
                        );
                        setAlertsNonce((value) => value + 1);
                      })
                      .catch((error: unknown) =>
                        pushToast(error instanceof Error ? error.message : "Không đổi được theo dõi."),
                      );
                  }}
                >
                  {option.label}
                </Button>
              ))}
            </div>
          </div>
          {watch.data.alerts.length > 0 && (
            <div className="queue-result" role="status">
              <p>
                <strong>{watch.data.alerts.length} chương mới</strong> kể từ lần kiểm tra trước:
              </p>
              <ul>
                {watch.data.alerts.slice(0, 8).map((alert) => (
                  <li key={alert.chapterId}>{alert.title}</li>
                ))}
                {watch.data.alerts.length > 8 && <li className="muted">…</li>}
              </ul>
              <div className="settings-actions">
                <Button
                  variant="primary"
                  onClick={() => {
                    const ids = watch.data.alerts.map((alert) => alert.chapterId);
                    setSelected((prev) => new Set([...prev, ...ids]));
                    pushToast(`Đã chọn ${ids.length} chương mới — bấm "Tải" để xếp hàng.`);
                  }}
                >
                  Chọn tất cả chương mới
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => {
                    void localServices.libraryV2
                      .ackAlerts(mangaId)
                      .then(() => setAlertsNonce((value) => value + 1))
                      .catch(() => undefined);
                  }}
                >
                  Đã xem hết
                </Button>
              </div>
            </div>
          )}
        </section>
      )}

      {history.state === "success" && history.data.total > 0 && (
        <section className="panel" aria-label="Lịch sử đọc">
          <div className="chapter-toolbar">
            <div>
              <h2 className="panel__title">Đã đọc gần đây ({history.data.total})</h2>
            </div>
            <Button
              variant="ghost"
              onClick={() => {
                void desktopClearReadHistory(mangaId)
                  .then((cleared) => {
                    pushToast(`Đã xóa ${cleared} mục lịch sử đọc.`);
                    setAlertsNonce((value) => value + 1);
                  })
                  .catch(() => pushToast("Không xóa được lịch sử đọc."));
              }}
            >
              Xóa lịch sử đọc
            </Button>
          </div>
          <ul className="continue-local">
            {history.data.items.map((entry) => (
              <li key={entry.chapterId} className="continue-local__row">
                <span className="continue-local__title">
                  {entry.chapterNumber !== null ? `Chương ${entry.chapterNumber} — ` : ""}
                  {entry.chapterTitle}
                </span>
                <span className="muted">{new Date(entry.readAt).toLocaleString()}</span>
                <Button to={`/read/${entry.chapterId}`} variant="ghost">
                  Đọc lại
                </Button>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className="panel" aria-label="Chương đã tải offline">
        <div className="section-header section-header--compact">
          <h2 className="panel__title">Đã có file ({downloadedIds.size})</h2>
          {downloaded.state === "success" && downloaded.data.length > 0 && (
            <Input
              label="Tìm chương đã tải"
              type="search"
              placeholder="Tên hoặc số chương (bỏ dấu vẫn khớp)…"
              value={downloadedQuery}
              onChange={(event) => setDownloadedQuery(event.target.value)}
            />
          )}
        </div>
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
            {visibleDownloaded.length === 0 && (
              <p className="muted">Không chương nào khớp bộ lọc “{downloadedQuery}”.</p>
            )}
            <ul className="chapter-select-list">
              {visibleDownloaded.map((chapter) => (
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
                    <StatusBadge tone="error">Thiếu file — cần tải lại</StatusBadge>
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
        <div className="lib-actions">
          <Button
            variant="ghost"
            onClick={() => setEraseOpen(true)}
            aria-label={`Xóa ${detail.title} khỏi Thư viện`}
          >
            <Trash2 size={14} aria-hidden="true" />
            Xóa truyện khỏi Thư viện
          </Button>
        </div>
        {eraseOpen && (
          <MangaDeleteDialog
            mangaId={detail.id}
            mangaTitle={detail.title}
            downloadedChapters={downloadedIds.size}
            onClose={() => setEraseOpen(false)}
            onDeleted={eraseOutcomeToast}
          />
        )}
      </section>

      <section className="panel" aria-label="Tải chương">
        <div className="section-header section-header--compact">
          <h2 className="panel__title">Tải chương</h2>
          <div className="chip-row">
            <Input
              label="Tìm chương"
              type="search"
              placeholder="Tên hoặc số chương (bỏ dấu vẫn khớp)…"
              value={queueQuery}
              onChange={(event) => {
                setQueueQuery(event.target.value);
                setChapterLimit(CHAPTER_PAGE);
              }}
            />
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
            {actionable.length > 0 && (
              <span
                className="chip-row"
                role="group"
                aria-label="Tải nhanh chương chưa tải"
              >
                <Button
                  variant="secondary"
                  disabled={!canDownload || submitting}
                  onClick={() => quickDownload(5)}
                >
                  Tải 5 chương tiếp
                </Button>
                <Button
                  variant="secondary"
                  disabled={!canDownload || submitting}
                  onClick={() => quickDownload(10)}
                >
                  Tải 10 chương tiếp
                </Button>
                <Button
                  variant="secondary"
                  disabled={!canDownload || submitting}
                  onClick={() => quickDownload(null)}
                >
                  Tải tất cả ({actionable.length})
                </Button>
              </span>
            )}
            <Button
              variant="ghost"
              disabled={cancelAllBusy}
              onClick={() => {
                setCancelAllBusy(true);
                void localServices.downloads
                  .cancelForManga(mangaId)
                  .then((accepted) =>
                    pushToast(
                      accepted > 0
                        ? `Đã yêu cầu hủy ${accepted} tác vụ đang chạy.`
                        : "Không có tác vụ nào đang chạy cho truyện này.",
                    ),
                  )
                  .catch((error: unknown) =>
                    pushToast(error instanceof Error ? error.message : "Không hủy được tác vụ."),
                  )
                  .finally(() => {
                    setCancelAllBusy(false);
                    bumpDomain("downloads");
                  });
              }}
            >
              Hủy mọi tác vụ của truyện
            </Button>
          </div>
        </div>
        {!canDownload && (
          <p className="panel__description">
Nguồn này chỉ đọc được metadata — không hỗ trợ tải trang.
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
        ) : visibleActionable.length === 0 ? (
          <p className="muted">Không chương nào khớp bộ lọc “{queueQuery}”.</p>
        ) : (
          <>
          <ul className="chapter-select-list">
            {shownActionable.map((chapter) => (
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
                    {chapter.number !== null ? `Chương ${chapter.number}` : "Không rõ số"}
                    {chapter.downloaded ? " · đã có file" : ""}
                  </span>
                </label>
              </li>
            ))}
          </ul>
          {visibleActionable.length > shownActionable.length && (
            <div className="settings-actions">
              <Button variant="secondary" onClick={() => setChapterLimit((n) => n + CHAPTER_PAGE)}>
                Hiện thêm (còn {visibleActionable.length - shownActionable.length})
              </Button>
            </div>
          )}
          </>
        )}
        <p className="muted">Định dạng xuất theo Settings → Output (mặc định CBZ).</p>
      </section>

      {dropActive && (
        <div className="drop-overlay" role="status">
          Thả tệp .cbz vào đây để nhập chương
        </div>
      )}
    </div>
  );
}
