import { History, Search, Trash2, X } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import { localServices } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";
import type { ReadHistoryDto, RecentSearchDto } from "../../types/backend";

const READ_PAGE_SIZE = 20;

function SearchRow({ query }: { query: string }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function remove() {
    setBusy(true);
    setError(null);
    try {
      await localServices.history.removeSearch(query);
      bumpDomain("history");
    } catch (caught: unknown) {
      setError(normalizeIpcError(caught).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <li className="history-row">
      <Search size={14} aria-hidden="true" />
      <Link to={`/discover?q=${encodeURIComponent(query)}`} className="history-row__text">
        {query}
      </Link>
      {error && (
        <span className="queue-error" role="alert">
          {error}
        </span>
      )}
      <Button
        variant="ghost"
        iconOnly
        aria-label={`Xóa ${query} khỏi lịch sử`}
        disabled={busy}
        onClick={() => void remove()}
      >
        <X size={14} aria-hidden="true" />
      </Button>
    </li>
  );
}

function ReadRow({ entry }: { entry: ReadHistoryDto }) {
  return (
    <li className="history-row">
      <History size={14} aria-hidden="true" />
      <Link to={`/read/${encodeURIComponent(entry.chapterId)}`} className="history-row__text">
        <span className="history-row__title">{entry.mangaTitle}</span>
        <span className="muted history-row__meta">{entry.chapterTitle}</span>
      </Link>
    </li>
  );
}

export default function HistoryPage() {
  const historyVersion = useDomainVersion("history");
  const searches = useAsyncData<RecentSearchDto[]>(
    () => localServices.history.listSearches(),
    [historyVersion],
  );
  const read = useAsyncData(
    () => localServices.history.listRead({ limit: READ_PAGE_SIZE, offset: 0 }),
    [historyVersion],
  );
  const [clearing, setClearing] = useState(false);
  const [clearError, setClearError] = useState<string | null>(null);

  async function clearSearches() {
    setClearing(true);
    setClearError(null);
    try {
      await localServices.history.clearSearches();
      bumpDomain("history");
    } catch (caught: unknown) {
      setClearError(normalizeIpcError(caught).message);
    } finally {
      setClearing(false);
    }
  }

  return (
    <div className="page-stack">
      <section className="panel" aria-label="Tìm kiếm gần đây">
        <div className="section-header">
          <h2 className="panel__title">
            <Search size={16} aria-hidden="true" /> Tìm kiếm gần đây
          </h2>
          {searches.state === "success" && searches.data.length > 0 && (
            <Button
              variant="secondary"
              disabled={clearing}
              onClick={() => void clearSearches()}
            >
              <Trash2 size={14} aria-hidden="true" /> Xóa tất cả
            </Button>
          )}
        </div>
        {clearError && (
          <p className="queue-error" role="alert">
            {clearError}
          </p>
        )}
        {searches.state === "loading" && <Skeleton height="6rem" label="Đang tải lịch sử tìm kiếm" />}
        {searches.state === "error" && (
          <ErrorState
            title="Không đọc được lịch sử tìm kiếm"
            description={normalizeIpcError(searches.error).message}
          />
        )}
        {searches.state === "success" && searches.data.length === 0 && (
          <EmptyState
            title="Chưa tìm kiếm gì"
            description="Câu truy vấn bạn bấm Tìm kiếm trên Khám phá sẽ xuất hiện ở đây."
          />
        )}
        {searches.state === "success" && searches.data.length > 0 && (
          <ul className="history-list">
            {searches.data.map((entry) => (
              <SearchRow key={entry.query} query={entry.query} />
            ))}
          </ul>
        )}
      </section>

      <section className="panel" aria-label="Chương đã đọc gần đây">
        <div className="section-header">
          <h2 className="panel__title">
            <History size={16} aria-hidden="true" /> Chương đã đọc gần đây
          </h2>
        </div>
        {read.state === "loading" && <Skeleton height="6rem" label="Đang tải lịch sử đọc" />}
        {read.state === "error" && (
          <ErrorState
            title="Không đọc được lịch sử đọc"
            description={normalizeIpcError(read.error).message}
          />
        )}
        {read.state === "success" && read.data.items.length === 0 && (
          <EmptyState
            title="Chưa đọc chương nào"
            description="Truyện mở bằng trình đọc offline sẽ được ghi lại ở đây."
          />
        )}
        {read.state === "success" && read.data.items.length > 0 && (
          <>
            <ul className="history-list">
              {read.data.items.map((entry) => (
                <ReadRow key={entry.chapterId} entry={entry} />
              ))}
            </ul>
            {read.data.total > read.data.items.length && (
              <p className="muted">
                Đang hiển thị {read.data.items.length}/{read.data.total} bản ghi gần nhất.
              </p>
            )}
          </>
        )}
      </section>
    </div>
  );
}
