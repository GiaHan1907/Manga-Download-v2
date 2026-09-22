import { Star } from "lucide-react";
import { useState } from "react";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Input from "../../components/ui/Input";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import Tooltip from "../../components/ui/Tooltip";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useServicesVersion } from "../../shared/hooks/useServices";
import { localServices } from "../../services/provider";
import { useDebouncedValue } from "../discover/hooks/useDebouncedValue";

const PAGE_SIZE = 20;

export default function LibraryPage() {
  const version = useServicesVersion();
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [retryNonce, setRetryNonce] = useState(0);
  const [actionError, setActionError] = useState<string | null>(null);
  const debouncedQuery = useDebouncedValue(query, 300);

  const page = useAsyncData(
    () => localServices.library.listMangas({ query: debouncedQuery || undefined, limit: PAGE_SIZE, offset }),
    [debouncedQuery, offset, retryNonce, version],
  );

  return (
    <div className="page-stack">
      <div className="section-header">
        <div>
          <p className="muted">
            Library <StatusBadge tone="success">Local — dữ liệu thật (SQLite)</StatusBadge>
          </p>
          <p className="muted">
            Danh sách sẽ có bản ghi khi Phase 05 đăng ký manga từ nguồn; Phase 04 không tự thêm dữ liệu.
          </p>
        </div>
        <Input
          label="Lọc theo tên"
          type="search"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setOffset(0);
          }}
        />
      </div>

      {page.state === "loading" && (
        <div className="card-grid" aria-busy="true">
          <Skeleton height="16rem" />
          <Skeleton height="16rem" />
        </div>
      )}

      {page.state === "error" && (
        <ErrorState
          title="Không đọc được Library từ database"
          description={`${normalizeIpcError(page.error).message} — Empty và lỗi DB là hai trạng thái khác nhau.`}
          action={
            <Button
              variant="primary"
              onClick={() => {
                setRetryNonce((value) => value + 1);
              }}
            >
              Thử lại
            </Button>
          }
        />
      )}

      {page.state === "success" && page.data.items.length === 0 && (
        <EmptyState
          title="Thư viện local đang trống"
          description="Chưa có manga nào được đăng ký vào database. Catalog demo 8 truyện ở tab Discover KHÔNG phải dữ liệu ở đây."
          action={<Button to="/discover" variant="secondary">Xem catalog demo (Discover)</Button>}
        />
      )}

      {page.state === "success" && page.data.items.length > 0 && (
        <>
          <div className="card-grid">
            {page.data.items.map((manga) => (
              <div key={manga.id} className="library-item">
                <MangaCard
                  title={manga.title}
                  subtitle={`${manga.chapterCount} chương đã đăng ký · ${manga.author ?? "không rõ tác giả"}`}
                  status={manga.status}
                />
                <div className="library-item__actions">
                  <Button
                    variant={manga.favorite ? "primary" : "ghost"}
                    aria-pressed={manga.favorite}
                    aria-label={`${manga.favorite ? "Bỏ yêu thích" : "Yêu thích"} ${manga.title}`}
                    onClick={async () => {
                      setActionError(null);
                      try {
                        await localServices.library.setFavorite(manga.id, !manga.favorite);
                        setRetryNonce((value) => value + 1);
                      } catch (error) {
                        setActionError(`${manga.title}: ${normalizeIpcError(error).message}`);
                      }
                    }}
                  >
                    <Star size={14} aria-hidden="true" />
                    {manga.favorite ? "Đang yêu thích" : "Yêu thích"}
                  </Button>
                  <Tooltip content="Manga Detail/Reader cho dữ liệu local nối từ Phase 05–07.">
                    <Button disabled aria-label={`Mở ${manga.title} — chưa khả dụng`}>
                      Mở Detail
                    </Button>
                  </Tooltip>
                </div>
              </div>
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

      {actionError && (
        <p className="queue-error" role="alert">
          Không cập nhật được favorite — {actionError}
        </p>
      )}
    </div>
  );
}
