import { LayoutGrid, List, Play, Star, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import Button from "../../components/ui/Button";
import Dialog from "../../components/ui/Dialog";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Input from "../../components/ui/Input";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import Tooltip from "../../components/ui/Tooltip";
import type { MangaDetail, MangaSummary } from "../../domain/manga";
import { services } from "../../services/provider";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useServicesVersion } from "../../shared/hooks/useServices";

interface LibraryEntry {
  summary: MangaSummary;
  detail: MangaDetail;
  favorite: boolean;
}

type SortMode = "recent" | "title";

export default function LibraryPage() {
  const version = useServicesVersion();
  const [view, setView] = useState<"grid" | "list">("grid");
  const [query, setQuery] = useState("");
  const [genre, setGenre] = useState("");
  const [status, setStatus] = useState("");
  const [sortMode, setSortMode] = useState<SortMode>("recent");
  const [removing, setRemoving] = useState<LibraryEntry | null>(null);

  const entries = useAsyncData<LibraryEntry[]>(async () => {
    const summaries = await services.library.listManga();
    return Promise.all(
      summaries.map(async (summary) => ({
        summary,
        detail: (await services.catalog.getManga(summary.id)) ?? ({ ...summary, description: "", chapters: [] } as MangaDetail),
        favorite: await services.library.isFavorite(summary.id),
      })),
    );
  }, [version]);

  const genres = useMemo(() => {
    if (entries.state !== "success") return [];
    return [...new Set(entries.data.flatMap((entry) => entry.summary.genres))].sort();
  }, [entries]);

  const filtered = useMemo(() => {
    if (entries.state !== "success") return [];
    const q = query.trim().toLowerCase();
    const list = entries.data.filter(
      (entry) =>
        (!q || entry.summary.title.toLowerCase().includes(q)) &&
        (!genre || entry.summary.genres.includes(genre)) &&
        (!status || entry.summary.status === status),
    );
    if (sortMode === "title") {
      return [...list].sort((a, b) => a.summary.title.localeCompare(b.summary.title, "vi"));
    }
    return [...list].reverse();
  }, [entries, query, genre, status, sortMode]);

  if (entries.state === "loading") {
    return (
      <div className="card-grid" aria-busy="true">
        <Skeleton height="18rem" />
        <Skeleton height="18rem" />
      </div>
    );
  }
  if (entries.state === "error") {
    return <ErrorState title="Không tải được Library demo" description="Dịch vụ dữ liệu mẫu lỗi. Reload để thử lại." />;
  }

  return (
    <div className="page-stack">
      <div className="library-toolbar">
        <Input label="Lọc theo tên" type="search" value={query} onChange={(event) => setQuery(event.target.value)} />
        <label className="input-field">
          <span className="input-field__label">Thể loại</span>
          <select className="input" value={genre} onChange={(event) => setGenre(event.target.value)}>
            <option value="">Tất cả</option>
            {genres.map((name) => (
              <option key={name} value={name}>{name}</option>
            ))}
          </select>
        </label>
        <label className="input-field">
          <span className="input-field__label">Trạng thái</span>
          <select className="input" value={status} onChange={(event) => setStatus(event.target.value)}>
            <option value="">Tất cả</option>
            <option value="ongoing">ongoing</option>
            <option value="completed">completed</option>
            <option value="unknown">unknown</option>
          </select>
        </label>
        <label className="input-field">
          <span className="input-field__label">Sắp xếp</span>
          <select className="input" value={sortMode} onChange={(event) => setSortMode(event.target.value as SortMode)}>
            <option value="recent">Thêm gần đây</option>
            <option value="title">Tên A→Z</option>
          </select>
        </label>
        <div className="library-toolbar__view" role="group" aria-label="Chế độ hiển thị">
          <Button variant={view === "grid" ? "primary" : "ghost"} iconOnly aria-label="Xem dạng lưới" aria-pressed={view === "grid"} onClick={() => setView("grid")}>
            <LayoutGrid size={16} aria-hidden="true" />
          </Button>
          <Button variant={view === "list" ? "primary" : "ghost"} iconOnly aria-label="Xem dạng danh sách" aria-pressed={view === "list"} onClick={() => setView("list")}>
            <List size={16} aria-hidden="true" />
          </Button>
        </div>
      </div>

      {filtered.length === 0 ? (
        <EmptyState
          title="Library demo trống"
          description="Không mục nào khớp bộ lọc, hoặc bạn đã gỡ hết mục khỏi thư viện mẫu."
          action={<Button onClick={() => { setQuery(""); setGenre(""); setStatus(""); }}>Xóa bộ lọc</Button>}
        />
      ) : (
        <div className={view === "grid" ? "card-grid" : "library-list"}>
          {filtered.map((entry) => {
            const sampleChapter = entry.detail.chapters.find((chapter) => chapter.hasSamplePages);
            return (
              <div key={entry.summary.id} className="library-item">
                <Link to={`/manga/${entry.summary.id}`} className="card-link" aria-label={`Mở ${entry.summary.title}`}>
                  <MangaCard
                    title={entry.summary.title}
                    subtitle={`${entry.detail.chapters.length} chương demo · ${entry.summary.genres.join(", ")}`}
                    status={entry.summary.status}
                    coverUrl={entry.summary.coverAsset}
                  />
                </Link>
                <div className="library-item__actions">
                  {sampleChapter ? (
                    <Button to={`/reader/${entry.summary.id}/${sampleChapter.id}`} variant="secondary">
                      <Play size={14} aria-hidden="true" />
                      Read
                    </Button>
                  ) : (
                    <Tooltip content="Chưa có trang mẫu đóng gói cho manga này trong bản demo.">
                      <Button disabled aria-label={`Read không khả dụng cho ${entry.summary.title}`}>
                        <Play size={14} aria-hidden="true" />
                        Read
                      </Button>
                    </Tooltip>
                  )}
                  <Button
                    variant={entry.favorite ? "primary" : "ghost"}
                    iconOnly
                    aria-label={entry.favorite ? `Bỏ yêu thích ${entry.summary.title}` : `Yêu thích ${entry.summary.title}`}
                    aria-pressed={entry.favorite}
                    onClick={() => void services.library.toggleFavorite(entry.summary.id)}
                  >
                    <Star size={16} aria-hidden="true" />
                  </Button>
                  <Button variant="ghost" iconOnly aria-label={`Gỡ ${entry.summary.title} khỏi thư viện mẫu`} onClick={() => setRemoving(entry)}>
                    <Trash2 size={16} aria-hidden="true" />
                  </Button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      <Dialog
        open={removing !== null}
        title="Gỡ khỏi thư viện mẫu?"
        description={removing ? `"${removing.summary.title}" sẽ bị gỡ khỏi mock state. Không có file nào bị xóa trên đĩa.` : undefined}
        onClose={() => setRemoving(null)}
        footer={
          <>
            <Button onClick={() => setRemoving(null)}>Hủy</Button>
            <Button
              variant="primary"
              onClick={async () => {
                if (removing) {
                  await services.library.removeFromLibrary(removing.summary.id);
                }
                setRemoving(null);
              }}
            >
              Gỡ (chỉ mock)
            </Button>
          </>
        }
      >
        <p className="muted">Hành vi Phase 03: chỉ cập nhật state demo trong phiên.</p>
      </Dialog>
    </div>
  );
}
