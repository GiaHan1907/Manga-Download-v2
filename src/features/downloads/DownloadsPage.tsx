import { RotateCcw, X } from "lucide-react";
import { useMemo, useState } from "react";
import Button from "../../components/ui/Button";
import DownloadCard from "../../components/ui/DownloadCard";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Input from "../../components/ui/Input";
import Skeleton from "../../components/ui/Skeleton";
import type { DemoDownloadTask, DownloadAction, DownloadStatus } from "../../domain/download";
import { allowedActions, taskProgressPercent } from "../../domain/download";
import { services } from "../../services/provider";
import { useCatalogData } from "../../shared/hooks/useCatalogData";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useServicesVersion } from "../../shared/hooks/useServices";

type StatusFilter = "all" | DownloadStatus;

const FILTERS: { id: StatusFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "queued", label: "Queued" },
  { id: "paused", label: "Paused" },
  { id: "completed", label: "Completed" },
  { id: "failed", label: "Failed" },
];

const statusTone: Record<DownloadStatus, "success" | "warning" | "error" | "neutral"> = {
  completed: "success",
  queued: "warning",
  paused: "neutral",
  failed: "error",
  cancelled: "neutral",
};

const actionLabels: Record<DownloadAction, string> = {
  pause: "Pause",
  resume: "Resume",
  retry: "Retry",
  cancel: "Cancel",
};

export default function DownloadsPage() {
  const version = useServicesVersion();
  const [filter, setFilter] = useState<StatusFilter>("all");
  const [query, setQuery] = useState("");
  const [newestFirst, setNewestFirst] = useState(false);
  const [transitionError, setTransitionError] = useState<string | null>(null);

  const catalog = useCatalogData();
  const tasks = useAsyncData(() => services.downloads.listTasks(), [version]);

  const visible = useMemo(() => {
    if (tasks.state !== "success" || catalog.state !== "success") {
      return [];
    }
    const q = query.trim().toLowerCase();
    const list = tasks.data.filter((task) => {
      if (filter !== "all" && task.status !== filter) {
        return false;
      }
      if (!q) {
        return true;
      }
      const manga = catalog.data.mangaById.get(task.mangaId);
      const chapter = catalog.data.chapterById.get(task.chapterId);
      return `${manga?.title ?? ""} ${chapter?.title ?? ""}`.toLowerCase().includes(q);
    });
    return newestFirst ? [...list].reverse() : list;
  }, [tasks, catalog, filter, query, newestFirst]);

  if (tasks.state === "loading" || catalog.state === "loading") {
    return <Skeleton height="12rem" label="Đang tải hàng đợi demo" />;
  }
  if (tasks.state === "error" || catalog.state === "error") {
    return <ErrorState title="Không tải được hàng đợi demo" description="Dịch vụ mock lỗi. Reload để thử lại." />;
  }

  async function runTransition(taskId: string, action: DownloadAction) {
    setTransitionError(null);
    try {
      await services.downloads.transition(taskId, action);
    } catch (error) {
      setTransitionError(error instanceof Error ? error.message : "Transition bị từ chối.");
    }
  }

  const counts = FILTERS.map((item) => ({
    ...item,
    count: item.id === "all" ? tasks.data.length : tasks.data.filter((task) => task.status === item.id).length,
  }));

  return (
    <div className="page-stack">
      <section className="panel" aria-label="Tổng quan hàng đợi demo">
        <h2 className="panel__title">Hàng đợi demo — {tasks.data.length} tác vụ</h2>
        <p className="panel__description">
          Trạng thái do người dùng chuyển đổi trong mock state. Không có tốc độ tải, dung lượng hay file thật nào.
        </p>
        <div className="chip-row">
          {counts.map((item) => (
            <button
              key={item.id}
              type="button"
              className={`filter-chip${filter === item.id ? " filter-chip--active" : ""}`}
              aria-pressed={filter === item.id}
              onClick={() => setFilter(item.id)}
            >
              {item.label} ({item.count})
            </button>
          ))}
        </div>
      </section>

      <div className="downloads-toolbar">
        <Input label="Tìm theo tên truyện/chương" type="search" value={query} onChange={(event) => setQuery(event.target.value)} />
        <Button variant="ghost" onClick={() => setNewestFirst((value) => !value)}>
          Sắp xếp: {newestFirst ? "mới nhất trước" : "thứ tự hàng đợi"}
        </Button>
      </div>

      {transitionError && <p className="queue-error" role="alert">{transitionError}</p>}

      {visible.length === 0 ? (
        <EmptyState
          title="Không có tác vụ demo khớp bộ lọc"
          description="Đổi bộ lọc, hoặc thêm chương từ Manga Detail để tạo tác vụ demo."
          action={<Button to="/discover" variant="secondary">Tới Discover</Button>}
        />
      ) : (
        <div className="downloads-grid">
          {visible.map((task: DemoDownloadTask) => {
            const manga = catalog.data.mangaById.get(task.mangaId);
            const chapter = catalog.data.chapterById.get(task.chapterId);
            return (
              <DownloadCard
                key={task.id}
                title={manga?.title ?? task.mangaId}
                description={`${chapter?.title ?? task.chapterId} · ${task.completedPages}/${task.totalPages} trang mô phỏng`}
                progress={taskProgressPercent(task)}
                status={`demo: ${task.status}`}
                statusTone={statusTone[task.status]}
              >
                {allowedActions(task.status).map((action) => (
                  <Button
                    key={action}
                    variant={action === "cancel" ? "ghost" : "secondary"}
                    onClick={() => void runTransition(task.id, action)}
                  >
                    {action === "cancel" && <X size={14} aria-hidden="true" />}
                    {action === "retry" && <RotateCcw size={14} aria-hidden="true" />}
                    {actionLabels[action]}
                  </Button>
                ))}
                <Button to={`/manga/${task.mangaId}`} variant="ghost">
                  View manga
                </Button>
              </DownloadCard>
            );
          })}
        </div>
      )}
    </div>
  );
}
