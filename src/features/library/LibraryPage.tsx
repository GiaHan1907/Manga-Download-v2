import {
  BellRing,
  BookOpen,
  Check,
  Clock,
  Download,
  Heart,
  Layers,
  LayoutGrid,
  Pause,
  RefreshCw,
  Rows3,
  Search,
  ShieldAlert,
  SortDesc,
  Star,
  Trash2,
  Trophy,
} from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import { normalizeIpcError, libraryStats, storageStatus } from "../../shared/api/desktop";
import { watchRunNow } from "../../shared/api/desktop";
import { pushToast } from "../../components/ui/toastStore";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";
import { useDebouncedValue } from "../discover/hooks/useDebouncedValue";
import { localServices } from "../../services/provider";
import Dialog from "../../components/ui/Dialog";
import MangaDeleteDialog from "./MangaDeleteDialog";
import { formatBytes } from "../../types/downloads";
import { relativeAge } from "../../shared/lib/time";
import type {
  LibraryFilter,
  LibraryGenreDto,
  LibraryMangaDto,
  LibrarySort,
} from "../../types/library";

const PAGE_SIZE = 24;
const RAIL_SIZE = 3;
const TILE_SIZE = 6;

const SORTS: { id: LibrarySort; label: string }[] = [
  { id: "lastRead", label: "Đọc gần đây" },
  { id: "recentDownload", label: "Mới cập nhật" },
  { id: "title", label: "Tên A → Z" },
  { id: "chapters", label: "Nhiều chương đã tải" },
];

/// Every entry here is a real `library_list` filter, so each pill and each
/// quick-filter tile changes what the database returns.
const STATES: { id: LibraryFilter; label: string }[] = [
  { id: "all", label: "Tất cả" },
  { id: "downloaded", label: "Đã tải" },
  { id: "partial", label: "Tải dở" },
  { id: "reading", label: "Đang đọc" },
  { id: "completed", label: "Hoàn thành" },
  { id: "favorite", label: "Yêu thích" },
  { id: "alerts", label: "Có chương mới" },
  { id: "missing", label: "Thiếu file" },
];

const STATUS_LABEL: Record<string, string> = {
  ongoing: "Đang tiến hành",
  completed: "Đã hoàn thành",
  unknown: "Chưa rõ",
};

function readViewMode(): "grid" | "list" {
  try {
    return window.localStorage.getItem("library.viewMode") === "list" ? "list" : "grid";
  } catch {
    return "grid";
  }
}

function progressPercent(manga: LibraryMangaDto): number | null {
  const total = manga.progress?.pageTotal ?? 0;
  if (!manga.progress || total <= 0) {
    return null;
  }
  return Math.min(100, Math.round(((manga.progress.pageIndex + 1) / total) * 100));
}

function Cover({ manga, size }: { manga: LibraryMangaDto; size: "tile" | "wide" }) {
  const url = useCoverUrl(manga.sourceId, manga.sourceMangaId, manga.coverKey);
  if (!url) {
    return (
      <span className={`lib-cover lib-cover--empty lib-cover--${size}`} aria-hidden="true">
        <BookOpen size={size === "wide" ? 22 : 16} />
      </span>
    );
  }
  return <img className={`lib-cover lib-cover--${size}`} src={url} alt="" loading="lazy" />;
}

function GenreChips({ manga }: { manga: LibraryMangaDto }) {
  const genres = (manga.genres ?? []).slice(0, 3);
  if (genres.length === 0) {
    return null;
  }
  return (
    <span className="lib-chips">
      {genres.map((genre) => (
        <span key={genre} className="lib-chip">
          {genre}
        </span>
      ))}
    </span>
  );
}

function ContinueCard({ manga }: { manga: LibraryMangaDto }) {
  const percent = progressPercent(manga);
  return (
    <article className="lib-continue">
      <Link to={`/library/${manga.id}`} className="lib-continue__art" aria-label={`Mở ${manga.title}`}>
        <Cover manga={manga} size="wide" />
      </Link>
      <div className="lib-continue__body">
        <Link to={`/library/${manga.id}`} className="lib-continue__title">
          {manga.title}
        </Link>
        <span className="lib-badge lib-badge--reading">Đang đọc</span>
        <p className="lib-continue__chapter">
          <span>{manga.progress?.chapterTitle ?? "Chưa có chương nào offline"}</span>
          {percent !== null && <span className="lib-continue__pct">{percent}% trang</span>}
        </p>
        {percent !== null && (
          <span className="lib-progress" role="img" aria-label={`Đã đọc ${percent}%`}>
            <span className="lib-progress__bar" style={{ width: `${percent}%` }} />
          </span>
        )}
        <p className="muted lib-continue__meta">
          <GenreChips manga={manga} />
          {manga.lastReadAt && <span>Đọc {relativeAge(manga.lastReadAt)}</span>}
        </p>
        <div className="lib-actions">
          <Button to={`/library/${manga.id}`} variant="primary">
            <BookOpen size={14} aria-hidden="true" /> Đọc tiếp
          </Button>
        </div>
      </div>
    </article>
  );
}

function MangaTile({
  manga,
  onToggleFavorite,
  onErase,
  selectable = false,
  selected = false,
  onToggleSelect,
}: {
  manga: LibraryMangaDto;
  onToggleFavorite: (manga: LibraryMangaDto) => void;
  onErase: (manga: LibraryMangaDto) => void;
  selectable?: boolean;
  selected?: boolean;
  onToggleSelect?: (manga: LibraryMangaDto) => void;
}) {
  const offline = manga.downloadedChapters > 0;
  return (
    <article className="lib-tile">
      <Link to={`/library/${manga.id}`} className="lib-tile__art" aria-label={`Mở ${manga.title}`}>
        <Cover manga={manga} size="tile" />
        <span
          className={`lib-badge lib-badge--float ${offline ? "lib-badge--ok" : "lib-badge--warn"}`}
        >
          {offline ? "Đã tải" : "Chưa tải"}
        </span>
      </Link>
      <div className="lib-tile__body">
        <Link to={`/library/${manga.id}`} className="lib-tile__title">
          {manga.title}
        </Link>
        <p className="muted lib-tile__meta">
          {manga.downloadedChapters}/{manga.totalChapters} chương offline
          {manga.localSizeBytes ? ` · ${formatBytes(manga.localSizeBytes)}` : ""}
        </p>
        <GenreChips manga={manga} />
        <p className="muted lib-tile__status">
          {STATUS_LABEL[manga.status] ?? manga.status}
          {manga.lastDownloadAt ? ` · tải ${relativeAge(manga.lastDownloadAt)}` : ""}
        </p>
        {manga.hasMissing && (
          <p className="queue-error lib-tile__missing" role="status">
            <ShieldAlert size={12} aria-hidden="true" /> Có file đã mất
          </p>
        )}
        <div className="lib-actions">
          {selectable && (
            <label className="lib-tile__select">
              <input
                type="checkbox"
                checked={selected}
                aria-label={`Chọn ${manga.title}`}
                onChange={() => onToggleSelect?.(manga)}
              />
            </label>
          )}
          <Button to={`/library/${manga.id}`} variant="primary" className="lib-tile__open">
            <BookOpen size={14} aria-hidden="true" /> {offline ? "Đọc tiếp" : "Mở"}
          </Button>
          <Button
            variant={manga.favorite ? "secondary" : "ghost"}
            iconOnly
            aria-pressed={manga.favorite}
            aria-label={`${manga.favorite ? "Bỏ yêu thích" : "Yêu thích"} ${manga.title}`}
            onClick={() => onToggleFavorite(manga)}
          >
            <Heart size={15} aria-hidden="true" />
          </Button>
          <Button
            variant="ghost"
            iconOnly
            aria-label={`Xóa ${manga.title} khỏi Thư viện`}
            onClick={() => onErase(manga)}
          >
            <Trash2 size={15} aria-hidden="true" />
          </Button>
        </div>
      </div>
    </article>
  );
}

function StatTile({
  icon,
  value,
  label,
}: {
  icon: React.ReactNode;
  value: string;
  label: string;
}) {
  return (
    <div className="lib-stat">
      <span className="lib-stat__icon" aria-hidden="true">
        {icon}
      </span>
      <span className="lib-stat__text">
        <span className="lib-stat__value">{value}</span>
        <span className="muted lib-stat__label">{label}</span>
      </span>
    </div>
  );
}

export default function LibraryPage() {
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [sort, setSort] = useState<LibrarySort>("lastRead");
  const [filter, setFilter] = useState<LibraryFilter>("all");
  const [genre, setGenre] = useState("");
  const [view, setView] = useState<"grid" | "list">(readViewMode);
  const [retryNonce, setRetryNonce] = useState(0);
  const [actionError, setActionError] = useState<string | null>(null);
  const [eraseTarget, setEraseTarget] = useState<LibraryMangaDto | null>(null);
  const [reconcileNote, setReconcileNote] = useState<string | null>(null);
  // Bulk selection: a mode, not a permanent checkbox on every tile — the grid
  // is for browsing first, and a stray click on a cover must never arm a delete.
  const [selectMode, setSelectMode] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [bulkBusy, setBulkBusy] = useState(false);
  const [bulkNote, setBulkNote] = useState<string | null>(null);
  const [bulkConfirmDelete, setBulkConfirmDelete] = useState(false);
  const debouncedQuery = useDebouncedValue(query, 300);
  const libraryVersion = useDomainVersion("library");

  const args = {
    query: debouncedQuery || undefined,
    sort,
    filter,
    genre: genre || undefined,
  };
  const page = useAsyncData(
    () => localServices.libraryV2.list({ ...args, limit: PAGE_SIZE, offset }),
    [debouncedQuery, offset, sort, filter, genre, retryNonce, libraryVersion],
  );
  // Rails and the aside read the same tables with their own ordering; they are
  // unfiltered on purpose so "Tiếp tục đọc" never disappears behind a filter.
  const continuing = useAsyncData(
    () => localServices.libraryV2.list({ sort: "lastRead", filter: "reading", limit: RAIL_SIZE, offset: 0 }),
    [libraryVersion, retryNonce],
  );
  const recent = useAsyncData(
    () =>
      localServices.libraryV2.list({
        sort: "recentDownload",
        filter: "downloaded",
        limit: TILE_SIZE,
        offset: 0,
      }),
    [libraryVersion, retryNonce],
  );
  const genres = useAsyncData<LibraryGenreDto[]>(
    () => localServices.libraryV2.genres(),
    [libraryVersion, retryNonce],
  );
  const stats = useAsyncData(() => libraryStats(), [libraryVersion, retryNonce]);
  const storage = useAsyncData(() => storageStatus(), [libraryVersion, retryNonce]);
  const readHistory = useAsyncData(
    () => localServices.history.listRead({ limit: 3, offset: 0 }),
    [libraryVersion, retryNonce],
  );

  const items = page.state === "success" ? page.data.items : [];
  const reading = (continuing.state === "success" ? continuing.data.items : []).filter(
    (manga) => manga.progress !== null,
  );
  const newest = recent.state === "success" ? recent.data.items : [];
  const highlight = reading[0] ?? items[0];

  function toggleView() {
    setView((value) => {
      const next = value === "grid" ? "list" : "grid";
      try {
        window.localStorage.setItem("library.viewMode", next);
      } catch {
        /* preference only; not authoritative data */
      }
      return next;
    });
  }

  function toggleSelect(mangaId: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(mangaId)) {
        next.delete(mangaId);
      } else {
        next.add(mangaId);
      }
      return next;
    });
  }

  /// Removes the selection from the library, optionally with its files. Runs one
  /// manga at a time on purpose: each call is its own transaction, and a refusal
  /// (active downloads are rejected in Rust) must not abort the rest.
  async function runBulk(deleteFiles: boolean) {
    setBulkBusy(true);
    setBulkConfirmDelete(false);
    setBulkNote(null);
    const ids = [...selected];
    let removed = 0;
    let files = 0;
    const failures: string[] = [];
    for (const mangaId of ids) {
      try {
        const outcome = await localServices.libraryV2.removeManga({ mangaId, deleteFiles });
        removed += 1;
        files += outcome.filesRemoved ?? 0;
      } catch (error) {
        failures.push(normalizeIpcError(error).message);
      }
    }
    bumpDomain("library");
    bumpDomain("home");
    bumpDomain("favorites");
    setSelected(new Set());
    setSelectMode(false);
    setBulkNote(
      failures.length === 0
        ? `Đã gỡ ${removed} truyện${deleteFiles ? ` và xoá ${files} file` : " (giữ file trên đĩa)"}.`
        : `Đã gỡ ${removed} truyện, ${failures.length} truyện không gỡ được (${failures[0]}).`,
    );
    setBulkBusy(false);
  }

  async function toggleFavorite(manga: LibraryMangaDto) {
    setActionError(null);
    try {
      await localServices.library.setFavorite(manga.id, !manga.favorite);
      bumpDomain("favorites");
      bumpDomain("library");
      bumpDomain("home");
    } catch (error) {
      setActionError(`${manga.title}: ${normalizeIpcError(error).message}`);
    }
  }

  function onEraseResult(outcome: { chaptersRemoved: number; filesRemoved: number }) {
    pushToast(
      `Đã gỡ truyện khỏi Thư viện (${outcome.chaptersRemoved} chương)` +
        (outcome.filesRemoved > 0 ? ` · xóa ${outcome.filesRemoved} file.` : " · file giữ nguyên."),
    );
    bumpDomain("library");
    bumpDomain("home");
    bumpDomain("favorites");
    bumpDomain("downloads");
    setEraseTarget(null);
  }

  async function runReconcile() {
    setActionError(null);
    setReconcileNote(null);
    try {
      const report = await localServices.libraryV2.reconcile();
      setReconcileNote(
        `Kiểm tra ${report.checked} file: ${report.healthy} ổn, ${report.missing} thiếu, ${report.corrupted} lỗi.` +
          (report.missing + report.corrupted > 0 ? " — tải lại các chương bị đánh dấu." : ""),
      );
      setRetryNonce((value) => value + 1);
    } catch (error) {
      setActionError(normalizeIpcError(error).message);
    }
  }

  function runWatchCheck() {
    void watchRunNow()
      .then((report) =>
        pushToast(
          report.alertedChapters > 0
            ? `${report.alertedChapters} chương mới từ ${report.checked} truyện được kiểm tra.`
            : `Đã kiểm tra ${report.checked} truyện theo dõi — không có chương mới.${report.failed > 0 ? ` (${report.failed} nguồn lỗi, sẽ thử lại sau)` : ""}`,
        ),
      )
      .catch((error: unknown) => {
        const normalized = normalizeIpcError(error);
        pushToast(
          normalized.code === "DB_CONFLICT"
            ? "Vừa kiểm tra xong — chờ ~1 phút rồi thử lại."
            : normalized.message,
        );
      });
  }

  const stat = stats.state === "success" ? stats.data : null;
  const volume = storage.state === "success" ? storage.data : null;
  const usedBytes = volume?.libraryBytes ?? stat?.localSizeBytes ?? 0;
  const capacity = volume?.volumeTotalBytes ?? null;
  const usedPercent =
    capacity && capacity > 0 ? Math.min(100, Math.round((usedBytes / capacity) * 100)) : null;

  // Sorted on the raw instant, not the formatted string — "2 giờ trước" sorts
  // alphabetically before "1 ngày trước" and would order the feed backwards.
  const activity: { icon: React.ReactNode; text: string; at: number }[] = [];
  for (const entry of readHistory.state === "success" ? readHistory.data.items : []) {
    activity.push({
      icon: <BookOpen size={13} aria-hidden="true" />,
      text: `Đọc ${entry.mangaTitle} — ${entry.chapterTitle}`,
      at: Date.parse(entry.readAt) || 0,
    });
  }
  for (const manga of newest.slice(0, 3)) {
    if (manga.lastDownloadAt) {
      activity.push({
        icon: <Download size={13} aria-hidden="true" />,
        text: `Đã tải ${manga.title} — ${manga.downloadedChapters}/${manga.totalChapters} chương`,
        at: Date.parse(manga.lastDownloadAt) || 0,
      });
    }
  }
  activity.sort((a, b) => b.at - a.at);

  return (
    <div className="page-stack library-page">
      <section className="library-hero" aria-label="Thư viện của bạn">
        <div className="library-hero__top">
          <div>
            <h1 className="library-hero__title">
              Thư viện <span className="library-hero__accent">của bạn</span>
            </h1>
            <p className="muted">
              Quản lý truyện đã tải, tiếp tục đọc và thưởng thức mọi lúc, mọi nơi.
            </p>
          </div>
          {highlight ? (
            <span className="library-hero__art" aria-hidden="true">
              <Cover manga={highlight} size="wide" />
            </span>
          ) : null}
        </div>

        <div className="discover-search">
          <Search size={16} aria-hidden="true" />
          <input
            className="discover-search__input"
            type="search"
            aria-label="Tìm trong thư viện"
            placeholder="Tìm trong thư viện của bạn..."
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setOffset(0);
            }}
          />
          <div className="lib-viewswitch" role="group" aria-label="Dạng hiển thị">
            <button
              type="button"
              aria-pressed={view === "grid"}
              className={view === "grid" ? "lib-viewswitch__on" : ""}
              onClick={() => view !== "grid" && toggleView()}
            >
              <LayoutGrid size={14} aria-hidden="true" /> Lưới
            </button>
            <button
              type="button"
              aria-pressed={view === "list"}
              className={view === "list" ? "lib-viewswitch__on" : ""}
              onClick={() => view !== "list" && toggleView()}
            >
              <Rows3 size={14} aria-hidden="true" /> Danh sách
            </button>
          </div>
          <Button
            variant={selectMode ? "secondary" : "ghost"}
            onClick={() => {
              setSelectMode((on) => !on);
              setSelected(new Set());
              setBulkNote(null);
            }}
          >
            {selectMode ? "Xong" : "Chọn nhiều"}
          </Button>
        </div>

        <div className="discover-filters">
          {genres.state === "success" && genres.data.length > 0 && (
            <label className="filter-pill">
              <Layers size={13} aria-hidden="true" />
              <span>Thể loại:</span>
              <select
                value={genre}
                onChange={(event) => {
                  setGenre(event.target.value);
                  setOffset(0);
                }}
              >
                <option value="">Tất cả thể loại</option>
                {genres.data.map((entry) => (
                  <option key={entry.genre} value={entry.genre}>
                    {entry.genre} ({entry.mangaCount})
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="filter-pill">
            <Check size={13} aria-hidden="true" />
            <span>Trạng thái:</span>
            <select
              value={filter}
              onChange={(event) => {
                setFilter(event.target.value as LibraryFilter);
                setOffset(0);
              }}
            >
              {STATES.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <label className="filter-pill">
            <SortDesc size={13} aria-hidden="true" />
            <span>Sắp xếp:</span>
            <select
              value={sort}
              onChange={(event) => {
                setSort(event.target.value as LibrarySort);
                setOffset(0);
              }}
            >
              {SORTS.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <span className="muted lib-count">
            {page.state === "success" ? `Tổng ${page.data.total} truyện` : "Đang đếm…"}
          </span>
        </div>

        <div className="lib-tools">
          <Button variant="ghost" onClick={() => void runReconcile()}>
            <RefreshCw size={14} aria-hidden="true" /> Kiểm tra file
          </Button>
          <Button variant="ghost" onClick={runWatchCheck}>
            <BellRing size={14} aria-hidden="true" /> Kiểm tra chương mới
          </Button>
        </div>
        {reconcileNote && (
          <p className="panel__note" role="status">
            {reconcileNote}
          </p>
        )}
      </section>

      <div className="library-layout">
        <div className="library-main page-stack">
          {reading.length > 0 && (
            <section aria-label="Tiếp tục đọc">
              <div className="section-header">
                <h2>
                  <Clock size={16} aria-hidden="true" /> Tiếp tục đọc
                </h2>
              </div>
              <div className="lib-continue-row">
                {reading.map((manga) => (
                  <ContinueCard key={manga.id} manga={manga} />
                ))}
              </div>
            </section>
          )}

          <section aria-label="Tất cả truyện trong thư viện">
            <div className="section-header">
              <h2>
                <BookOpen size={16} aria-hidden="true" /> Tất cả truyện trong thư viện
              </h2>
            </div>

            {page.state === "loading" && (
              <div className="lib-tile-row" aria-busy="true">
                <Skeleton height="19rem" />
                <Skeleton height="19rem" />
                <Skeleton height="19rem" />
              </div>
            )}
            {page.state === "error" && (
              <ErrorState
                title="Không đọc được Library từ database"
                description={`${normalizeIpcError(page.error).message} — Thư viện trống và lỗi DB là hai trạng thái khác nhau.`}
                action={
                  <Button variant="primary" onClick={() => setRetryNonce((value) => value + 1)}>
                    Thử lại
                  </Button>
                }
              />
            )}
            {page.state === "success" && items.length === 0 && (
              <EmptyState
                title={filter === "all" && !genre ? "Thư viện local đang trống" : "Không có kết quả khớp bộ lọc"}
                description={
                  filter === "all" && !genre
                    ? "Chưa có truyện nào được lưu. Lưu từ Khám phá để nó xuất hiện ở đây."
                    : "Đổi bộ lọc hoặc xóa từ khóa tìm kiếm."
                }
                action={<Button to="/discover" variant="secondary">Tới Khám phá</Button>}
              />
            )}
            {page.state === "success" && items.length > 0 && (
              <>
                <div className={view === "grid" ? "lib-tile-row" : "library-list"}>
                  {view === "grid"
                    ? items.map((manga) => (
                        <MangaTile
                          key={manga.id}
                          manga={manga}
                          onToggleFavorite={(m) => void toggleFavorite(m)}
                          onErase={setEraseTarget}
                          selectable={selectMode}
                          selected={selected.has(manga.id)}
                          onToggleSelect={(m) => toggleSelect(m.id)}
                        />
                      ))
                    : items.map((manga) => (
                        <div key={manga.id} className="library-row">
                          <Link to={`/library/${manga.id}`} className="library-row__link">
                            <span className="library-row__title">{manga.title}</span>
                            <span className="muted">{manga.author ?? "—"}</span>
                            <span className="muted">
                              {manga.downloadedChapters}/{manga.totalChapters} offline
                              {manga.localSizeBytes ? ` · ${formatBytes(manga.localSizeBytes)}` : ""}
                            </span>
                            <span className="muted">
                              {manga.lastReadAt ? `đọc ${relativeAge(manga.lastReadAt)}` : "chưa đọc"}
                            </span>
                          </Link>
                          <Button
                            variant={manga.favorite ? "secondary" : "ghost"}
                            iconOnly
                            aria-label={`${manga.favorite ? "Bỏ yêu thích" : "Yêu thích"} ${manga.title}`}
                            onClick={() => void toggleFavorite(manga)}
                          >
                            <Star size={15} aria-hidden="true" />
                          </Button>
                        </div>
                      ))}
                </div>
                <div className="chapter-toolbar__footer">
                  <span className="muted">
                    Trang {Math.floor(offset / PAGE_SIZE) + 1} · tổng {page.data.total} bản ghi
                  </span>
                  <div className="settings-actions">
                    <Button
                      variant="ghost"
                      disabled={offset === 0}
                      onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
                    >
                      Trước
                    </Button>
                    <Button
                      variant="ghost"
                      disabled={offset + PAGE_SIZE >= page.data.total}
                      onClick={() => setOffset(offset + PAGE_SIZE)}
                    >
                      Sau
                    </Button>
                  </div>
                </div>
              </>
            )}
          </section>

      {selectMode && (
        <section className="panel bulk-bar" aria-label="Thao tác nhiều truyện">
          <div className="bulk-bar__row">
            <span>
              Đã chọn <strong>{selected.size}</strong> truyện
            </span>
            <Button
              variant="ghost"
              onClick={() =>
                setSelected(
                  new Set(
                    (page.state === "success" ? page.data.items : []).map((manga) => manga.id),
                  ),
                )
              }
            >
              Chọn cả trang
            </Button>
            <Button variant="ghost" disabled={selected.size === 0} onClick={() => setSelected(new Set())}>
              Bỏ chọn
            </Button>
            <Button
              variant="secondary"
              disabled={bulkBusy || selected.size === 0}
              onClick={() => void runBulk(false)}
            >
              Gỡ khỏi Thư viện (giữ file)
            </Button>
            <Button
              variant="primary"
              disabled={bulkBusy || selected.size === 0}
              onClick={() => setBulkConfirmDelete(true)}
            >
              Gỡ và xoá file
            </Button>
          </div>
        </section>
      )}
      {bulkNote && (
        // Outside the bar on purpose: the mode closes as soon as the run ends,
        // and the summary is exactly what the user needs to read then.
        <p className="muted" role="status">
          {bulkNote}
        </p>
      )}

          {newest.length > 0 && (
            <section aria-label="Mới thêm vào">
              <div className="section-header">
                <h2>
                  <Download size={16} aria-hidden="true" /> Mới thêm vào
                </h2>
              </div>
              <div className="lib-tile-row">
                {newest.map((manga) => (
                  <MangaTile
                    key={manga.id}
                    manga={manga}
                    onToggleFavorite={(m) => void toggleFavorite(m)}
                    onErase={setEraseTarget}
                  />
                ))}
              </div>
            </section>
          )}
        </div>

        <aside className="library-aside page-stack">
          <section className="panel" aria-label="Thống kê thư viện">
            <h2 className="panel__title">
              <Layers size={16} aria-hidden="true" /> Thống kê thư viện
            </h2>
            {stats.state === "loading" && <Skeleton height="6rem" label="Đang tải thống kê" />}
            {stats.state === "error" && (
              <p className="queue-error" role="alert">
                {normalizeIpcError(stats.error).message}
              </p>
            )}
            {stat && (
              <>
                <div className="lib-stats">
                  <StatTile
                    icon={<BookOpen size={15} aria-hidden="true" />}
                    value={String(stat.savedMangas)}
                    label="Tổng truyện"
                  />
                  <StatTile
                    icon={<Download size={15} aria-hidden="true" />}
                    value={(stat.totalChapters ?? 0).toLocaleString("vi-VN")}
                    label="Tổng chương"
                  />
                  <StatTile
                    icon={<Layers size={15} aria-hidden="true" />}
                    value={formatBytes(stat.localSizeBytes)}
                    label="Dung lượng"
                  />
                </div>
                {capacity !== null && usedPercent !== null ? (
                  <p className="muted lib-capacity">
                    <span className="lib-progress lib-progress--thin" role="img" aria-label={`Đã dùng ${usedPercent}% dung lượng ổ`}>
                      <span className="lib-progress__bar" style={{ width: `${usedPercent}%` }} />
                    </span>
                    <span className="lib-capacity__text">
                      Đã dùng {formatBytes(usedBytes)} / {formatBytes(capacity)} · {usedPercent}%
                    </span>
                  </p>
                ) : (
                  <p className="muted lib-capacity">
                    Không đọc được dung lượng ổ đĩa — chỉ hiển thị dung lượng thư viện
                    ({formatBytes(usedBytes)}).
                  </p>
                )}
              </>
            )}
          </section>

          {stat && (
            <section className="panel" aria-label="Bộ lọc nhanh">
              <h2 className="panel__title">
                <Check size={16} aria-hidden="true" /> Bộ lọc nhanh
              </h2>
              <div className="lib-quick">
                {(
                  [
                    { id: "all", label: "Tất cả", value: stat.savedMangas, icon: <LayoutGrid size={14} aria-hidden="true" /> },
                    { id: "downloaded", label: "Đã tải", value: stat.downloadedMangas ?? 0, icon: <Download size={14} aria-hidden="true" /> },
                    { id: "reading", label: "Đang đọc", value: stat.reading ?? 0, icon: <Clock size={14} aria-hidden="true" /> },
                    { id: "completed", label: "Hoàn thành", value: stat.completed ?? 0, icon: <Trophy size={14} aria-hidden="true" /> },
                    { id: "favorite", label: "Yêu thích", value: stat.favorites, icon: <Heart size={14} aria-hidden="true" /> },
                    { id: "missing", label: "Thiếu file", value: stat.missing ?? 0, icon: <ShieldAlert size={14} aria-hidden="true" /> },
                  ] as const
                ).map((entry) => (
                  <button
                    key={entry.id}
                    type="button"
                    className={`lib-quick__tile${filter === entry.id ? " lib-quick__tile--on" : ""}`}
                    aria-pressed={filter === entry.id}
                    onClick={() => {
                      setFilter(entry.id);
                      setOffset(0);
                    }}
                  >
                    <span className="lib-quick__icon" aria-hidden="true">
                      {entry.icon}
                    </span>
                    <span className="lib-quick__text">
                      <span className="lib-quick__label">{entry.label}</span>
                      <span className="muted lib-quick__value">{entry.value}</span>
                    </span>
                  </button>
                ))}
              </div>
              {stat.paused ? (
                <p className="muted lib-quick__note">
                  <Pause size={12} aria-hidden="true" /> {stat.paused} chương đang tạm dừng trong hàng đợi
                  — mở Trạm tải để tiếp tục.
                </p>
              ) : null}
            </section>
          )}

          {highlight && (
            <section className="panel" aria-label="Đọc tiếp nổi bật">
              <h2 className="panel__title">
                <BookOpen size={16} aria-hidden="true" /> Đọc tiếp nổi bật
              </h2>
              <div className="lib-feature">
                <Link to={`/library/${highlight.id}`} className="lib-feature__art" aria-label={`Mở ${highlight.title}`}>
                  <Cover manga={highlight} size="wide" />
                </Link>
                <div className="lib-feature__body">
                  <Link to={`/library/${highlight.id}`} className="lib-feature__title">
                    {highlight.title}
                  </Link>
                  <GenreChips manga={highlight} />
                  <p className="muted lib-feature__meta">
                    {highlight.downloadedChapters}/{highlight.totalChapters} chương offline
                  </p>
                  {highlight.description && (
                    <p className="lib-feature__desc">{highlight.description.slice(0, 220)}</p>
                  )}
                  <Button to={`/library/${highlight.id}`} variant="primary">
                    <BookOpen size={14} aria-hidden="true" /> Đọc tiếp
                  </Button>
                </div>
              </div>
            </section>
          )}

          <section className="panel" aria-label="Hoạt động gần đây">
            <h2 className="panel__title">
              <Clock size={16} aria-hidden="true" /> Hoạt động gần đây
            </h2>
            {activity.length === 0 ? (
              <p className="muted">Chưa có hoạt động nào được ghi lại.</p>
            ) : (              <ul className="lib-activity">
                {activity.slice(0, 5).map((entry, index) => (
                  <li key={`${entry.text}-${index}`}>
                    <span className="lib-activity__icon" aria-hidden="true">
                      {entry.icon}
                    </span>
                    <span className="lib-activity__text">{entry.text}</span>
                    <span className="muted lib-activity__age">{relativeAge(new Date(entry.at).toISOString())}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </aside>
      </div>

      {actionError && (
        <p className="queue-error" role="alert">
          {actionError}
        </p>
      )}
      <Dialog
        open={bulkConfirmDelete}
        title={`Gỡ ${selected.size} truyện và xoá file?`}
        description="File CBZ/PDF của những truyện này sẽ bị xoá khỏi đĩa. Metadata, yêu thích và tiến độ đọc cũng bị gỡ cùng. Không hoàn tác được."
        onClose={() => setBulkConfirmDelete(false)}
        footer={
          <>
            <Button variant="ghost" onClick={() => setBulkConfirmDelete(false)}>
              Hủy
            </Button>
            <Button variant="primary" disabled={bulkBusy} onClick={() => void runBulk(true)}>
              Xác nhận xoá file
            </Button>
          </>
        }
      >
        <p className="muted">
          Nếu chỉ muốn dọn danh sách mà giữ nội dung đã tải, chọn &quot;Gỡ khỏi Thư viện (giữ
          file)&quot;.
        </p>
      </Dialog>

      {eraseTarget && (
        <MangaDeleteDialog
          mangaId={eraseTarget.id}
          mangaTitle={eraseTarget.title}
          downloadedChapters={eraseTarget.downloadedChapters}
          onClose={() => setEraseTarget(null)}
          onDeleted={onEraseResult}
        />
      )}
    </div>
  );
}
