import { Compass, Link2 } from "lucide-react";
import { Link } from "react-router-dom";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import SystemStatus from "../system/SystemStatus";
import type { MangaSummary } from "../../domain/manga";
import type { DemoDownloadTask, DownloadStatus } from "../../domain/download";
import type { ReadingProgress } from "../../domain/reader";
import { services } from "../../services/provider";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useCatalogData } from "../../shared/hooks/useCatalogData";
import { useServicesVersion } from "../../shared/hooks/useServices";
import ContinueReading from "./components/ContinueReading";
import StatCard from "./components/StatCard";

const statusTone: Record<DownloadStatus, "success" | "warning" | "error" | "neutral"> = {
  completed: "success",
  queued: "warning",
  paused: "neutral",
  failed: "error",
  cancelled: "neutral",
};

interface DashboardState {
  library: MangaSummary[];
  tasks: DemoDownloadTask[];
  continueReading: (ReadingProgress & { totalPages: number }) | null;
}

export default function HomePage() {
  const version = useServicesVersion();
  const catalog = useCatalogData();
  const dashboard = useAsyncData<DashboardState>(async () => {
    const [library, tasks, progressList] = await Promise.all([
      services.library.listManga(),
      services.downloads.listTasks(),
      services.reader.listProgress(),
    ]);
    const latest = progressList[progressList.length - 1] ?? null;
    const totalPages = latest ? (await services.reader.getPages(latest.chapterId)).length : 0;
    return {
      library,
      tasks,
      continueReading: latest ? { ...latest, totalPages } : null,
    };
  }, [version]);

  if (catalog.state === "error" || dashboard.state === "error") {
    return (
      <ErrorState
        title="Không tải được bảng điều khiển demo"
        description="Dịch vụ dữ liệu mẫu trả về lỗi. Thử lại bằng cách reload giao diện."
      />
    );
  }

  if (catalog.state === "loading" || dashboard.state === "loading") {
    return (
      <div className="page-stack" aria-busy="true" aria-label="Đang tải bảng điều khiển">
        <Skeleton height="3rem" />
        <Skeleton height="8rem" />
        <Skeleton height="10rem" />
      </div>
    );
  }

  const recentAdditions = [...dashboard.data.library].reverse().slice(0, 4);
  const queuePreview = dashboard.data.tasks.slice(0, 4);
  const continueTitle = dashboard.data.continueReading
    ? (catalog.data.mangaById.get(dashboard.data.continueReading.mangaId)?.title ?? null)
    : null;

  return (
    <div className="page-stack">
      <section className="panel home-hero">
        <div>
          <h2 className="panel__title">Bảng điều khiển demo</h2>
          <p className="panel__description">
            Prototype Phase 03 — toàn bộ số liệu lấy từ fixture nội bộ, không có tải truyện hay tìm kiếm thật.
          </p>
        </div>
        <div className="home-hero__actions">
          <Button to="/discover" variant="primary">
            <Compass size={16} aria-hidden="true" />
            Explore Manga
          </Button>
          <Button to="/discover?tab=url" variant="secondary">
            <Link2 size={16} aria-hidden="true" />
            Import URL
          </Button>
        </div>
      </section>

      <section className="stats-grid" aria-label="Thống kê mẫu">
        <StatCard label="Manga trong catalog demo" value={String(catalog.data.allManga.length)} hint="Sample statistics" />
        <StatCard label="Chương demo" value={String(catalog.data.allChapters.length)} hint="Sample statistics" />
        <StatCard label="Tác vụ hàng đợi (demo)" value={String(dashboard.data.tasks.length)} hint="Simulated queue" />
        <StatCard label="Dung lượng lưu trữ" value="—" hint="Not connected in UI prototype" />
      </section>

      <ContinueReading progress={dashboard.data.continueReading} title={continueTitle} />

      <section aria-label="Thêm mới gần đây">
        <div className="section-header">
          <h2>Thêm gần đây (dữ liệu mẫu)</h2>
          <Button to="/library" variant="ghost">Xem Library</Button>
        </div>
        {recentAdditions.length === 0 ? (
          <p className="muted">Chưa có mục nào trong thư viện demo.</p>
        ) : (
          <div className="card-grid">
            {recentAdditions.map((manga) => (
              <Link key={manga.id} to={`/manga/${manga.id}`} className="card-link">
                <MangaCard title={manga.title} subtitle={manga.author ?? manga.sourceId} status={manga.status} coverUrl={manga.coverAsset} />
              </Link>
            ))}
          </div>
        )}
      </section>

      <section aria-label="Xem trước hàng đợi demo">
        <div className="section-header">
          <h2>Hàng đợi tải (demo)</h2>
          <Button to="/downloads" variant="ghost">Mở Downloads</Button>
        </div>
        {queuePreview.length === 0 ? (
          <p className="muted">Chưa có tác vụ demo nào.</p>
        ) : (
          <ul className="queue-preview">
            {queuePreview.map((task) => {
              const manga = catalog.data.mangaById.get(task.mangaId);
              const chapter = catalog.data.chapterById.get(task.chapterId);
              return (
                <li key={task.id}>
                  <Link to="/downloads" className="queue-preview__item">
                    <span>{manga?.title ?? task.mangaId} — {chapter?.title ?? task.chapterId}</span>
                    <StatusBadge tone={statusTone[task.status]}>{task.status}</StatusBadge>
                  </Link>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <SystemStatus />
    </div>
  );
}
