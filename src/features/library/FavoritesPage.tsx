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
import { localServices } from "../../services/provider";

const PAGE_SIZE = 20;

export default function FavoritesPage() {
  const [retryNonce, setRetryNonce] = useState(0);
  const favorites = useAsyncData(
    () => localServices.library.listFavorites({ limit: PAGE_SIZE, offset: 0 }),
    [retryNonce],
  );

  return (
    <div className="page-stack">
      <p className="muted">
        Favorites <StatusBadge tone="success">Local — SQLite</StatusBadge> · khác với ★ demo ở Manga Detail (sandbox Phase 03).
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
          title="Chưa có mục yêu thích local"
          description="Bật ★ trong Library khi database đã có manga đăng ký (Phase 05)."
          action={<Button to="/library" variant="secondary">Tới Library</Button>}
        />
      )}

      {favorites.state === "success" && favorites.data.items.length > 0 && (
        <div className="card-grid">
          {favorites.data.items.map((manga) => (
            <div key={manga.id} className="library-item">
              <MangaCard
                title={manga.title}
                subtitle={`${manga.chapterCount} chương đã đăng ký`}
                status={manga.status}
              />
              <div className="library-item__actions">
                <Button
                  onClick={() => {
                    void localServices.library
                      .setFavorite(manga.id, false)
                      .then(() => setRetryNonce((value) => value + 1));
                  }}
                >
                  <Star size={14} aria-hidden="true" />
                  Bỏ yêu thích
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
