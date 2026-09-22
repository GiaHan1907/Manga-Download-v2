import { Star } from "lucide-react";
import { useState } from "react";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";
import { localServices } from "../../services/provider";

const PAGE_SIZE = 20;

export default function FavoritesPage() {
  const [retryNonce, setRetryNonce] = useState(0);
  const [offset, setOffset] = useState(0);
  const [actionError, setActionError] = useState<string | null>(null);
  const favoritesVersion = useDomainVersion("favorites");
  const favorites = useAsyncData(
    () => localServices.library.listFavorites({ limit: PAGE_SIZE, offset }),
    [retryNonce, offset, favoritesVersion],
  );

  async function unfavorite(mangaId: string) {
    setActionError(null);
    try {
      await localServices.library.setFavorite(mangaId, false);
      bumpDomain("favorites");
      bumpDomain("library");
      bumpDomain("home");
    } catch (error) {
      setActionError(normalizeIpcError(error).message);
    }
  }

  return (
    <div className="page-stack">
      <p className="muted">
        Favorites <StatusBadge tone="success">Local — SQLite</StatusBadge> · khác với ★ demo ở Manga
        Detail (sandbox chỉ tồn tại ở bản dev).
      </p>

      {favorites.state === "loading" && <Skeleton height="16rem" label="Đang tải danh sách yêu thích local" />}

      {favorites.state === "error" && (
        <ErrorState
          title="Không đọc được Favorites từ database"
          description={normalizeIpcError(favorites.error).message}
          action={
            <Button variant="primary" onClick={() => setRetryNonce((value) => value + 1)}>
              Thử lại
            </Button>
          }
        />
      )}

      {favorites.state === "success" && favorites.data.items.length === 0 && (
        <EmptyState
          title={offset > 0 ? "Trang này không còn bản ghi" : "Chưa có mục yêu thích local"}
          description="Bật ★ trong Library khi database đã có manga đăng ký (Phase 05)."
          action={<Button to="/library" variant="secondary">Tới Library</Button>}
        />
      )}

      {favorites.state === "success" && favorites.data.items.length > 0 && (
        <>
          <div className="card-grid">
            {favorites.data.items.map((manga) => (
              <div key={manga.id} className="library-item">
                <MangaCard
                  to={`/library/${manga.id}`}
                  title={manga.title}
                  subtitle={`${manga.chapterCount} chương đã đăng ký`}
                  status={manga.status}
                />
                <div className="library-item__actions">
                  <Button onClick={() => void unfavorite(manga.id)}>
                    <Star size={14} aria-hidden="true" />
                    Bỏ yêu thích
                  </Button>
                </div>
              </div>
            ))}
          </div>
          <div className="chapter-toolbar__footer">
            <span className="muted">
              Trang {Math.floor(offset / PAGE_SIZE) + 1} · tổng {favorites.data.total} bản ghi
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
                disabled={offset + PAGE_SIZE >= favorites.data.total}
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
          Không bỏ yêu thích được — {actionError}
        </p>
      )}
    </div>
  );
}
