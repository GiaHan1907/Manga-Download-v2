import { Compass, Download, Link2 } from "lucide-react";
import { useState } from "react";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import SystemStatus from "../system/SystemStatus";
import {
  listLocalDownloadTasks,
  libraryStats,
  normalizeIpcError,
} from "../../shared/api/desktop";
import type { DownloadTaskDto, LibraryStatsDto } from "../../types/backend";
import { formatBytes } from "../../types/downloads";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useDomainVersion } from "../../shared/hooks/useInvalidation";
import LocalContinueReading from "./components/LocalContinueReading";
import StatCard from "./components/StatCard";

const STAT_TONE: Partial<Record<DownloadTaskDto["status"], "success" | "warning" | "error" | "neutral">> = {
  completed: "success",
  failed: "error",
  cancelled: "neutral",
};

export default function HomePage() {
  const homeVersion = useDomainVersion("home");
  const [retryNonce, setRetryNonce] = useState(0);
  const stats = useAsyncData<LibraryStatsDto>(() => libraryStats(), [homeVersion, retryNonce]);
  const recentTasks = useAsyncData<DownloadTaskDto[]>(
    async () => (await listLocalDownloadTasks({ limit: 5, offset: 0 })).items,
    [homeVersion, retryNonce],
  );

  if (stats.state === "loading" || recentTasks.state === "loading") {
    return (
      <div className="page-stack" aria-busy="true" aria-label="Đang tải bảng điều khiển">
        <Skeleton height="3rem" />
        <Skeleton height="8rem" />
        <Skeleton height="10rem" />
      </div>
    );
  }
  if (stats.state === "error") {
    return (
      <ErrorState
        title="Không tải được số liệu thư viện"
        description={normalizeIpcError(stats.error).message}
        action={
          <Button variant="primary" onClick={() => setRetryNonce((value) => value + 1)}>
            Thử lại
          </Button>
        }
      />
    );
  }

  return (
    <div className="page-stack">
      <LocalContinueReading />
      <section className="panel home-hero">
        <div>
          <h2 className="panel__title">Bảng điều khiển</h2>
          <p className="panel__description">
            Số liệu dưới đây là dữ liệu thật từ SQLite + đĩa máy bạn (Phase 04–07). Không có fixture
            ở màn này.
          </p>
        </div>
        <div className="home-hero__actions">
          <Button to="/discover" variant="primary">
            <Compass size={16} aria-hidden="true" />
            Khám phá nguồn thật
          </Button>
          <Button to="/discover?tab=url" variant="secondary">
            <Link2 size={16} aria-hidden="true" />
            Import URL
          </Button>
        </div>
      </section>

      <section className="stats-grid" aria-label="Thống kê local">
        <StatCard label="Truyện đã lưu" value={String(stats.data.savedMangas)} hint="SQLite — mangas" />
        <StatCard
          label="Chương offline"
          value={String(stats.data.downloadedChapters)}
          hint="Artifact đã kiểm chứng"
        />
        <StatCard
          label="Dung lượng thư viện"
          value={stats.data.localSizeBytes > 0 ? formatBytes(stats.data.localSizeBytes) : "0 B"}
          hint="approximate — từ DB sizes"
        />
        <StatCard label="Yêu thích" value={String(stats.data.favorites)} hint="SQLite — favorites" />
      </section>

      <section aria-label="Tác vụ tải gần đây">
        <div className="section-header">
          <h2>
            <Download size={16} aria-hidden="true" /> Tác vụ tải gần đây
          </h2>
          <Button to="/downloads" variant="ghost">
            Mở Downloads
          </Button>
        </div>
        {recentTasks.state === "error" && (
          <p className="queue-error" role="alert">
            {normalizeIpcError(recentTasks.error).message}
          </p>
        )}
        {recentTasks.state === "success" && recentTasks.data.length === 0 && (
          <p className="muted">Chưa có tác vụ tải nào. Lưu truyện từ Discover rồi chọn chương để tải.</p>
        )}
        {recentTasks.state === "success" && recentTasks.data.length > 0 && (
          <ul className="queue-preview">
            {recentTasks.data.map((task) => (
              <li key={task.id}>
                <div className="queue-preview__item">
                  <span>
                    {task.mangaTitle ?? task.chapterId} — {task.chapterTitle ?? task.chapterId}
                  </span>
                  <StatusBadge tone={STAT_TONE[task.status] ?? "warning"}>{task.status}</StatusBadge>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      <SystemStatus />
    </div>
  );
}
