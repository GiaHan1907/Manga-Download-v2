import { Star } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Input from "../../components/ui/Input";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useServicesVersion } from "../../shared/hooks/useServices";
import { localServices } from "../../services/provider";
import type { LibraryFilter, LibraryMangaDto, LibrarySort } from "../../types/library";
import { useDebouncedValue } from "../discover/hooks/useDebouncedValue";
import { formatBytes } from "../../types/downloads";

const PAGE_SIZE = 24;

const SORTS: { id: LibrarySort; label: string }[] = [
  { id: "lastRead", label: "Đọc gần đây" },
  { id: "recentDownload", label: "Tải gần đây" },
  { id: "title", label: "Tên" },
  { id: "chapters", label: "Số chương đã tải" },
];
const FILTERS: { id: LibraryFilter; label: string }[] = [
  { id: "all", label: "Tất cả" },
  { id: "downloaded", label: "Có nội dung offline" },
  { id: "partial", label: "Tải dở" },
  { id: "favorite", label: "Yêu thích" },
];

function readViewMode(): "grid" | "list" {
  try {
    return window.localStorage.getItem("library.viewMode") === "list" ? "list" : "grid";
  } catch {
    return "grid";
  }
}

function relativeTime(iso: string | null): string {
  if (!iso) {
    return "";
  }
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return date.toLocaleDateString();
}

export default function LibraryPage() {
  const version = useServicesVersion();
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [sort, setSort] = useState<LibrarySort>("lastRead");
  const [filter, setFilter] = useState<LibraryFilter>("all");
  const [view, setView] = useState<"grid" | "list">(readViewMode);
  const [retryNonce, setRetryNonce] = useState(0);
  const [actionError, setActionError] = useState<string | null>(null);
  const [reconcileNote, setReconcileNote] = useState<string | null>(null);
  const debouncedQuery = useDebouncedValue(query, 300);

  const page = useAsyncData(
    () =>
      localServices.libraryV2.list({
        query: debouncedQuery || undefined,
        sort,
        filter,
        limit: PAGE_SIZE,
        offset,
      }),
    [debouncedQuery, offset, sort, filter, retryNonce, version],
  );

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

  async function toggleFavorite(manga: LibraryMangaDto) {
    setActionError(null);
    try {
      await localServices.library.setFavorite(manga.id, !manga.favorite);
      setRetryNonce((value) => value + 1);
    } catch (error) {
      setActionError(`${manga.title}: ${normalizeIpcError(error).message}`);
    }
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

  const items = page.state === "success" ? page.data.items : [];

  return (
    <div className="page-stack">
      <div className="section-header">
        <div>
          <p className="muted">
            Thư viện <StatusBadge tone="success">Local — SQLite thật</StatusBadge>
          </p>
          <p className="muted">
            Truyện đã lưu từ Discover; chương hiện “offline” khi file CBZ/thư mục đã được
            kiểm chứng. Không có dữ liệu demo ở màn này.
          </p>
        </div>
        <Input
          label="Tìm theo tên/tác giả"
          type="search"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setOffset(0);
          }}
        />
      </div>

      <div className="library-toolbar" role="toolbar" aria-label="Điều khiển thư viện">
        <label className="library-field">
          <span className="muted">Sắp xếp</span>
          <select value={sort} onChange={(event) => { setSort(event.target.value as LibrarySort); setOffset(0); }}>
            {SORTS.map((option) => (
              <option key={option.id} value={option.id}>{option.label}</option>
            ))}
          </select>
        </label>
        <label className="library-field">
          <span className="muted">Lọc</span>
          <select value={filter} onChange={(event) => { setFilter(event.target.value as LibraryFilter); setOffset(0); }}>
            {FILTERS.map((option) => (
              <option key={option.id} value={option.id}>{option.label}</option>
            ))}
          </select>
        </label>
        <Button variant="ghost" onClick={toggleView} aria-label="Đổi dạng lưới/danh sách">
          {view === "grid" ? "Dạng danh sách" : "Dạng lưới"}
        </Button>
        <Button variant="ghost" onClick={() => void runReconcile()}>
          Kiểm tra file
        </Button>
        <span className="muted library-count">
          {page.state === "success" ? `${page.data.total} truyện` : ""}
        </span>
      </div>

      {reconcileNote && <p className="panel__note" role="status">{reconcileNote}</p>}

      {page.state === "loading" && (
        <div className="card-grid" aria-busy="true">
          <Skeleton height="16rem" />
          <Skeleton height="16rem" />
        </div>
      )}

      {page.state === "error" && (
        <ErrorState
          title="Không đọc được Library từ database"
          description={`${normalizeIpcError(page.error).message} — Thư viện trống và lỗi DB là hai trạng thái khác nhau.`}
          action={<Button variant="primary" onClick={() => setRetryNonce((value) => value + 1)}>Thử lại</Button>}
        />
      )}

      {page.state === "success" && items.length === 0 && (
        <EmptyState
          title={filter === "all" ? "Thư viện local đang trống" : "Không có kết quả khớp bộ lọc"}
          description={
            filter === "all"
              ? "Chưa có truyện nào được lưu. Lưu từ Discover (nguồn thật) để nó xuất hiện ở đây."
              : "Đổi bộ lọc/sắp xếp, hoặc xóa từ khóa tìm kiếm."
          }
          action={<Button to="/discover" variant="secondary">Tới Discover</Button>}
        />
      )}

      {page.state === "success" && items.length > 0 && (
        <>
          <div className={view === "grid" ? "card-grid" : "library-list"}>
            {items.map((manga) => (
              <LibraryEntry
                key={manga.id}
                manga={manga}
                view={view}
                onToggleFavorite={() => void toggleFavorite(manga)}
              />
            ))}
          </div>
          <div className="chapter-toolbar__footer">
            <span className="muted">
              Trang {Math.floor(offset / PAGE_SIZE) + 1} · tổng {page.data.total} bản ghi
            </span>
            <div className="settings-actions">
              <Button variant="ghost" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}>
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

      {actionError && <p className="queue-error" role="alert">{actionError}</p>}
    </div>
  );
}

function LibraryEntry({
  manga,
  view,
  onToggleFavorite,
}: {
  manga: LibraryMangaDto;
  view: "grid" | "list";
  onToggleFavorite: () => void;
}) {
  const pct =
    manga.progress && (manga.progress.pageTotal ?? 0) > 0
      ? Math.min(
          100,
          Math.round(((manga.progress.pageIndex + 1) / (manga.progress.pageTotal ?? 0)) * 100),
        )
      : 0;
  const card = (
    <MangaCard
      to={`/library/${manga.id}`}
      title={manga.title}
      subtitle={`${manga.downloadedChapters}/${manga.totalChapters} chương offline · ${manga.author ?? "không rõ tác giả"}`}
      status={manga.status}
    />
  );
  return (
    <div className={view === "grid" ? "library-item" : "library-row"}>
      {view === "list" ? (
        <Link to={`/library/${manga.id}`} className="library-row__link">
          <span className="library-row__title">{manga.title}</span>
          <span className="muted">{manga.author ?? "—"}</span>
          <span className="muted">
            {manga.downloadedChapters}/{manga.totalChapters} offline
            {manga.localSizeBytes ? ` · ${formatBytes(manga.localSizeBytes)}` : ""}
          </span>
          <span className="muted">
            {manga.lastReadAt ? `đọc ${relativeTime(manga.lastReadAt)}` : "chưa đọc"}
          </span>
        </Link>
      ) : (
        card
      )}
      <p className="muted library-item__offline">
        {manga.hasMissing ? (
          <StatusBadge tone="error">Có file đã mất — cần tải lại</StatusBadge>
        ) : manga.downloadedChapters > 0 ? (
          <StatusBadge tone="success">{`${manga.downloadedChapters} chương offline`}</StatusBadge>
        ) : (
          <StatusBadge tone="neutral">Chưa tải chương nào</StatusBadge>
        )}
      </p>
      {manga.progress && (
        <p className="muted library-item__progress" role="status">
          Đang đọc · {pct}%
        </p>
      )}
      <div className="library-item__actions">
        <Button
          variant={manga.favorite ? "primary" : "ghost"}
          aria-pressed={manga.favorite}
          aria-label={`${manga.favorite ? "Bỏ yêu thích" : "Yêu thích"} ${manga.title}`}
          onClick={onToggleFavorite}
        >
          <Star size={14} aria-hidden="true" />
          {manga.favorite ? "Đang yêu thích" : "Yêu thích"}
        </Button>
        <Button to={`/library/${manga.id}`} variant="secondary" aria-label={`Mở ${manga.title}`}>
          Mở
        </Button>
      </div>
    </div>
  );
}
