import { Star } from "lucide-react";
import { Link } from "react-router-dom";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import { services } from "../../services/provider";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useServicesVersion } from "../../shared/hooks/useServices";

export default function FavoritesPage() {
  const version = useServicesVersion();
  const favorites = useAsyncData(() => services.library.listFavorites(), [version]);

  if (favorites.state === "loading") {
    return <Skeleton height="18rem" label="Đang tải danh sách yêu thích demo" />;
  }
  if (favorites.state === "error") {
    return <ErrorState title="Không tải được Favorites demo" description="Dịch vụ dữ liệu mẫu lỗi. Reload để thử lại." />;
  }

  if (favorites.data.length === 0) {
    return (
      <EmptyState
        title="Chưa có mục yêu thích mẫu"
        description="Bật ★ ở Manga Detail hoặc Library để thêm vào danh sách yêu thích demo."
        action={<Button to="/discover" variant="primary">Khám phá catalog demo</Button>}
      />
    );
  }

  return (
    <div className="page-stack">
      <p className="muted">Danh sách dùng chung mock state với Library — thay đổi đồng bộ ngay trong phiên.</p>
      <div className="card-grid">
        {favorites.data.map((manga) => (
          <div key={manga.id} className="library-item">
            <Link to={`/manga/${manga.id}`} className="card-link" aria-label={`Mở ${manga.title}`}>
              <MangaCard
                title={manga.title}
                subtitle={`${manga.author ?? "Không rõ tác giả"} · ${manga.genres.join(", ")}`}
                status={manga.status}
                coverUrl={manga.coverAsset}
              />
            </Link>
            <div className="library-item__actions">
              <Button variant="primary" onClick={() => void services.library.toggleFavorite(manga.id)}>
                <Star size={14} aria-hidden="true" />
                Bỏ yêu thích
              </Button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
