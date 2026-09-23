import { Ban, Pause, Play, RotateCcw } from "lucide-react";
import { useMemo, useState } from "react";
import Button from "../../components/ui/Button";
import DownloadCard from "../../components/ui/DownloadCard";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import { pushToast } from "../../components/ui/toastStore";
import type { DownloadTaskDto } from "../../types/backend";
import {
  allowedDownloadActions,
  formatBytes,
  formatEtaSeconds,
  formatSpeed,
  isActiveStatus,
  progressPercent,
} from "../../types/downloads";
import { useDownloadQueue } from "./useDownloadQueue";

type FilterId = "all" | "active" | "paused" | "completed" | "failed" | "cancelled";

const FILTERS: { id: FilterId; label: string; match: (s: DownloadTaskDto["status"]) => boolean }[] = [
  { id: "all", label: "Tất cả", match: () => true },
  { id: "active", label: "Đang chạy", match: (s) => isActiveStatus(s) },
  { id: "paused", label: "Tạm dừng", match: (s) => s === "paused" || s === "pausing" },
  { id: "completed", label: "Hoàn tất", match: (s) => s === "completed" },
  { id: "failed", label: "Lỗi", match: (s) => s === "failed" },
  { id: "cancelled", label: "Đã hủy", match: (s) => s === "cancelled" },
];

const statusLabel: Record<DownloadTaskDto["status"], string> = {
  queued: "Trong hàng đợi",
  resolving: "Đang phân giải",
  downloading: "Đang tải",
  pausing: "Đang dừng",
  paused: "Tạm dừng",
  verifying: "Đang kiểm tra",
  exporting: "Đang đóng gói",
  completed: "Hoàn tất",
  failed: "Thất bại",
  cancelled: "Đã hủy",
};

const statusTone: Record<DownloadTaskDto["status"], "success" | "warning" | "error" | "neutral"> = {
  queued: "warning",
  resolving: "warning",
  downloading: "warning",
  pausing: "warning",
  paused: "neutral",
  verifying: "warning",
  exporting: "warning",
  completed: "success",
  failed: "error",
  cancelled: "neutral",
};

const actionIcon: Record<"pause" | "resume" | "retry" | "cancel", typeof Pause> = {
  pause: Pause,
  resume: Play,
  retry: RotateCcw,
  cancel: Ban,
};

const actionLabel: Record<"pause" | "resume" | "retry" | "cancel", string> = {
  pause: "Tạm dừng",
  resume: "Tiếp tục",
  retry: "Thử lại",
  cancel: "Hủy",
};

const actionSuccessToast: Record<"pause" | "resume" | "retry" | "cancel", string> = {
  pause: "Đã tạm dừng tác vụ.",
  resume: "Đã tiếp tục tác vụ.",
  retry: "Đã xếp hàng lại tác vụ.",
  cancel: "Đã hủy tác vụ.",
};

function runAction(actions: ReturnType<typeof useDownloadQueue>["actions"], taskId: string, action: "pause" | "resume" | "retry" | "cancel") {
  void actions[action](taskId)
    .then(() => pushToast(actionSuccessToast[action]))
    .catch(() => undefined); // inline per-card feedback handles rejection
}

export default function DownloadsPage() {
  const {
    tasks,
    live,
    speedHistory,
    etaSeconds,
    actionErrors,
    loading,
    error,
    actions,
    refresh,
    total,
    offset,
    goPage,
  } = useDownloadQueue();
  const [filter, setFilter] = useState<FilterId>("all");

  const visible = useMemo(() => {
    const matcher = FILTERS.find((item) => item.id === filter) ?? FILTERS[0];
    return tasks.filter((task) => matcher.match(task.status));
  }, [tasks, filter]);

  const counts = useMemo(
    () => Object.fromEntries(FILTERS.map((item) => [item.id, tasks.filter((task) => item.match(task.status)).length])) as Record<FilterId, number>,
    [tasks],
  );

  if (loading && tasks.length === 0) {
    return (
      <div className="page-stack" aria-busy="true">
        <Skeleton height="6rem" label="Đang tải hàng đợi" />
        <Skeleton height="18rem" />
      </div>
    );
  }
  if (error && tasks.length === 0) {
    return <ErrorState title="Không tải được hàng đợi tải" description={error} onRetry={refresh} />;
  }

  return (
    <div className="page-stack">
      <section className="panel" aria-label="Tổng quan hàng đợi tải">
        <h2 className="panel__title">
          Hàng đợi tải — {tasks.length}/{total} tác vụ (trang {Math.floor(offset / 100) + 1})
        </h2>
        <p className="panel__description">
          Trạng thái, tiến độ và file đều là dữ liệu thật từ engine Rust + SQLite. Bộ lọc áp dụng trên
          trang đang xem; tốc độ chỉ hiện khi đo được; chương chỉ báo &quot;hoàn tất&quot; sau khi mục
          tin đã được kiểm tra toàn vẹn.
        </p>
        <div className="chip-row">
          {FILTERS.map((item) => (
            <button
              key={item.id}
              type="button"
              className={`filter-chip${filter === item.id ? " filter-chip--active" : ""}`}
              aria-pressed={filter === item.id}
              onClick={() => setFilter(item.id)}
            >
              {item.label} ({counts[item.id]})
            </button>
          ))}
          <Button variant="ghost" onClick={refresh}>
            Làm mới
          </Button>
        </div>
      </section>

      {error && <p className="queue-error" role="alert">{error}</p>}

      {visible.length === 0 ? (
        tasks.length > 0 ? (
          <EmptyState
            title="Không có tác vụ khớp bộ lọc"
            description={`Trong ${tasks.length} tác vụ của trang hiện tại không có trạng thái đang chọn. Đổi bộ lọc hoặc bấm Làm mới.`}
            action={<Button variant="secondary" onClick={() => setFilter("all")}>Xem tất cả</Button>}
          />
        ) : (
          <EmptyState
            title="Chưa có tác vụ tải"
            description="Mở một truyện đã lưu trong Library, chọn chương rồi bấm Tải để xếp hàng."
            action={<Button to="/library" variant="primary">Tới Library</Button>}
          />
        )
      ) : (
        <div className="downloads-grid">
          {visible.map((task) => {
            const liveInfo = live[task.id];
            const speed = statusTone[task.status] === "warning" ? formatSpeed(liveInfo?.speed ?? null) : null;
            // ETA is only truthful while pages are actively advancing —
            // a paused/queued task must not keep showing a stale estimate.
            const etaActive = task.status === "resolving" || task.status === "downloading";
            const parts = [
              `${task.completedPages}/${task.totalPages} trang`,
              liveInfo && liveInfo.bytes > 0 ? formatBytes(liveInfo.bytes) : null,
              speed,
              etaActive ? formatEtaSeconds(etaSeconds[task.id]) : null,
              task.targetFormat === "folder" ? "thư mục ảnh" : "CBZ",
            ].filter(Boolean);
            return (
              <DownloadCard
                key={task.id}
                title={task.mangaTitle ?? task.chapterId}
                description={`${task.chapterTitle ?? task.chapterId} · ${parts.join(" · ")}`}
                progress={progressPercent(task)}
                status={statusLabel[task.status]}
                statusTone={statusTone[task.status]}
                speedHistory={speedHistory[task.id]}
                actionError={actionErrors[task.id]}
              >
                {task.status === "failed" && task.errorMessage && (
                  <p className="download-card__error" role="status">
                    {task.errorCode ? `[${task.errorCode}] ` : ""}
                    {task.errorMessage}
                  </p>
                )}
                {task.status === "completed" && task.outputRelpath && (
                  <span className="download-card__meta">Đã xác minh: {task.outputRelpath}</span>
                )}
                {allowedDownloadActions(task.status).map((action) => {
                  const Icon = actionIcon[action];
                  return (
                    <Button
                      key={action}
                      variant={action === "cancel" ? "ghost" : "secondary"}
                      onClick={() => runAction(actions, task.id, action)}
                    >
                      <Icon size={14} aria-hidden="true" />
                      {actionLabel[action]}
                    </Button>
                  );
                })}
              </DownloadCard>
            );
          })}
        </div>
      )}

      {total > 100 && (
        <div className="chapter-toolbar__footer">
          <span className="muted">
            Trang {Math.floor(offset / 100) + 1}/{Math.ceil(total / 100)} · tổng {total} tác vụ
          </span>
          <div className="settings-actions">
            <Button variant="ghost" disabled={offset === 0} onClick={() => goPage(offset - 100)}>
              Trước
            </Button>
            <Button
              variant="ghost"
              disabled={offset + 100 >= total}
              onClick={() => goPage(offset + 100)}
            >
              Sau
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
