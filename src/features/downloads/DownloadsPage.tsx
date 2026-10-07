import { Ban, Eraser, FolderOpen, Link, Pause, Play, RotateCcw } from "lucide-react";
import { useMemo, useState } from "react";
import Button from "../../components/ui/Button";
import Dialog from "../../components/ui/Dialog";
import DownloadCard from "../../components/ui/DownloadCard";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import { pushToast } from "../../components/ui/toastStore";
import {
  normalizeIpcError,
  queueChapterDownloadByUrl,
  revealDownload,
} from "../../shared/api/desktop";
import type { DownloadTaskDto } from "../../types/backend";
import {
  allowedDownloadActions,
  canRemoveTask,
  formatBytes,
  formatEtaSeconds,
  formatSpeed,
  DOWNLOAD_STATUS_LABEL,
  isActiveStatus,
  progressPercent,
} from "../../types/downloads";
import { useDownloadQueue } from "./useDownloadQueue";

type FilterId = "queue" | "all" | "active" | "paused" | "completed" | "failed" | "cancelled";

const FILTERS: { id: FilterId; label: string; match: (s: DownloadTaskDto["status"]) => boolean }[] = [
  // Default view: the work that is still work. A finished chapter leaves the
  // queue the moment it completes, but its row stays in the database (it is the
  // offline proof) and under "Hoàn thành".
  { id: "queue", label: "Hàng đợi", match: (s) => s !== "completed" },
  { id: "all", label: "Tất cả", match: () => true },
  { id: "active", label: "Đang chạy", match: (s) => isActiveStatus(s) },
  { id: "paused", label: "Tạm dừng", match: (s) => s === "paused" || s === "pausing" },
  { id: "completed", label: "Hoàn thành", match: (s) => s === "completed" },
  { id: "failed", label: "Lỗi", match: (s) => s === "failed" },
  { id: "cancelled", label: "Đã hủy", match: (s) => s === "cancelled" },
];

/// The order the queue actually runs in, mirrored from `list_tasks`: a raised
/// priority first, then chapter number small→large, unnumbered chapters last,
/// then submission time. Re-sorting here also keeps the view stable when an
/// event arrives for a task that was not on the loaded page.
function byQueueOrder(a: DownloadTaskDto, b: DownloadTaskDto): number {
  if (a.priority !== b.priority) return b.priority - a.priority;
  const left = a.chapterNumber ?? null;
  const right = b.chapterNumber ?? null;
  if (left === null || right === null) {
    if (left !== right) return left === null ? 1 : -1;
  } else if (left !== right) {
    return left - right;
  }
  return a.createdAt.localeCompare(b.createdAt);
}

const statusLabel = DOWNLOAD_STATUS_LABEL;

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

const PURGE_STATUS_LABELS: Record<string, string> = {
  completed: "Hoàn thành",
  failed: "Lỗi",
  cancelled: "Đã hủy",
};

export default function DownloadsPage() {
  const {
    queuePaused,
    setQueuePaused,
    tasks,
    live,
    speedHistory,
    etaSeconds,
    actionErrors,
    loading,
    error,
    actions,
    removeTask,
    retryAllFailed,
    clearHistory,
    refresh,
    total,
    offset,
    goPage,
  } = useDownloadQueue();
  const [filter, setFilter] = useState<FilterId>("queue");
  const [bulkBusy, setBulkBusy] = useState(false);
  const [purgeOpen, setPurgeOpen] = useState(false);
  const [purgeStatuses, setPurgeStatuses] = useState<string[]>(["failed", "cancelled"]);
  // "Toàn bộ thời gian" is the default on purpose: the age limit is a filter
  // the user adds, not a precondition. With 7 days selected by default,
  // clearing today's failed tasks silently deleted nothing.
  const [purgeOlderThanDays, setPurgeOlderThanDays] = useState<number | null>(null);
  const [linkText, setLinkText] = useState("");
  const [linkBusy, setLinkBusy] = useState(false);

  const visible = useMemo(() => {
    const matcher = FILTERS.find((item) => item.id === filter) ?? FILTERS[0];
    return tasks.filter((task) => matcher.match(task.status)).sort(byQueueOrder);
  }, [tasks, filter]);

  const counts = useMemo(
    () => Object.fromEntries(FILTERS.map((item) => [item.id, tasks.filter((task) => item.match(task.status)).length])) as Record<FilterId, number>,
    [tasks],
  );

  function retryEverythingFailed() {
    setBulkBusy(true);
    void retryAllFailed()
      .then((count) => pushToast(count > 0 ? `Đã xếp hàng lại ${count} tác vụ lỗi.` : "Không còn tác vụ lỗi nào."))
      .catch((err: unknown) => pushToast(err instanceof Error ? err.message : "Bulk retry thất bại."))
      .finally(() => setBulkBusy(false));
  }

  function toggleQueueGate() {
    const next = !queuePaused;
    setBulkBusy(true);
    void setQueuePaused(next)
      .then((moved) =>
        pushToast(
          next
            ? `Đã tạm dừng hàng đợi — ${moved} tác vụ chờ được giữ nguyên.`
            : `Đã tiếp tục — ${moved} tác vụ vào lại hàng đợi.`,
        ),
      )
      .catch((err: unknown) => pushToast(err instanceof Error ? err.message : "Không đổi được trạng thái hàng đợi."))
      .finally(() => setBulkBusy(false));
  }

  function confirmPurge() {
    if (purgeStatuses.length === 0) {
      return;
    }
    setBulkBusy(true);
    void clearHistory({
      statuses: purgeStatuses,
      olderThanDays: purgeOlderThanDays ?? undefined,
    })
      .then((outcome) => {
        const limit =
          purgeOlderThanDays === null ? "" : ` chỉ mục cũ hơn ${purgeOlderThanDays} ngày`;
        if (outcome.deleted === 0) {
          pushToast(
            `Không có mục nào khớp${limit === "" ? " bộ lọc" : limit} — lịch sử vẫn nguyên.`,
          );
        } else {
          pushToast(`Đã xóa ${outcome.deleted} mục lịch sử${limit}. File truyện không bị xóa.`);
        }
        setPurgeOpen(false);
      })
      .catch((err: unknown) => pushToast(err instanceof Error ? err.message : "Không xóa được lịch sử."))
      .finally(() => setBulkBusy(false));
  }

  // Paste-a-chapter-link quick download — resolution, auto-save of an
  // unseen manga and queueing all happen in Rust; this only shows the
  // path-free result.
  async function downloadViaLink() {
    const url = linkText.trim();
    if (url === "") {
      pushToast("Dán link chương truyện trước đã.");
      return;
    }
    setLinkBusy(true);
    try {
      const result = await queueChapterDownloadByUrl({ url });
      if (result.tasks.length > 0) {
        pushToast("Đã xếp hàng 1 chương từ link.");
        setLinkText("");
      } else if (result.reused.length > 0) {
        pushToast("Chương này đã có trong hàng đợi.");
        setLinkText("");
      } else {
        pushToast(result.errors[0]?.message ?? "Không xếp hàng được từ link này.");
      }
      refresh();
    } catch (err) {
      pushToast(normalizeIpcError(err).message);
    } finally {
      setLinkBusy(false);
    }
  }

  // Rust opens Explorer (artifact selected, or library root with no id);
  // no path ever comes back to this side.
  function openLocation(taskId?: string) {
    void revealDownload(taskId ? { taskId } : {}).catch((err: unknown) =>
      pushToast(normalizeIpcError(err).message),
    );
  }

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
          Hàng đợi tải — {visible.length}/{total} tác vụ (trang {Math.floor(offset / 100) + 1})
        </h2>
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
          {counts.failed > 0 && (
            <Button variant="secondary" disabled={bulkBusy} onClick={retryEverythingFailed}>
              <RotateCcw size={14} aria-hidden="true" />
              Thử lại tất cả lỗi ({counts.failed})
            </Button>
          )}
          <Button
            variant={queuePaused ? "primary" : "secondary"}
            disabled={bulkBusy}
            onClick={() => void toggleQueueGate()}
          >
            {queuePaused ? "Tiếp tục toàn bộ" : "Dừng toàn bộ"}
          </Button>
          {queuePaused && (
            <span className="status-badge status-badge--warning" role="status">
              Hàng đợi đang tạm dừng
            </span>
          )}
          <Button variant="ghost" disabled={bulkBusy} onClick={() => setPurgeOpen(true)}>
            <Eraser size={14} aria-hidden="true" />
            Dọn lịch sử
          </Button>
          <Button variant="ghost" onClick={() => openLocation()}>
            <FolderOpen size={14} aria-hidden="true" />
            Mở thư mục tải
          </Button>
        </div>
        <div className="discover-controls block-spaced">
          <input
            className="input-field"
            type="url"
            value={linkText}
            onChange={(event) => setLinkText(event.target.value)}
            placeholder="Dán link chương (VD: https://site/manga/truyen/chuong-1/)"
            aria-label="Link chương cần tải"
            onKeyDown={(event) => {
              if (event.key === "Enter") void downloadViaLink();
            }}
          />
          <Button variant="secondary" disabled={linkBusy} onClick={() => void downloadViaLink()}>
            <Link size={14} aria-hidden="true" />
            {linkBusy ? "Đang xử lý…" : "Tải từ link"}
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
                  <>
                    <span className="download-card__meta">
                      Đã xác minh: {task.outputRelpath}
                    </span>
                    <Button variant="secondary" onClick={() => openLocation(task.id)}>
                      <FolderOpen size={14} aria-hidden="true" />
                      Mở nơi lưu
                    </Button>
                  </>
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
                {canRemoveTask(task.status) && (
                  <Button
                    variant="ghost"
                    onClick={() =>
                      void removeTask(task.id)
                        .then(() => pushToast("Đã gỡ tác vụ khỏi danh sách."))
                        .catch((err: unknown) =>
                          pushToast(
                            err instanceof Error ? err.message : "Không gỡ được tác vụ.",
                          ),
                        )
                    }
                  >
                    <Eraser size={14} aria-hidden="true" />
                    Gỡ khỏi danh sách
                  </Button>
                )}
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

      <Dialog
        open={purgeOpen}
        title="Dọn lịch sử tải"
        description="Chỉ xóa dòng lịch sử trong DB — không bao giờ xóa file truyện đã tải. Mục hoàn tất vẫn còn nguyên trong Thư viện."
        onClose={() => setPurgeOpen(false)}
        footer={
          <>
            <Button variant="ghost" onClick={() => setPurgeOpen(false)}>
              Hủy
            </Button>
            <Button
              variant="primary"
              disabled={bulkBusy || purgeStatuses.length === 0}
              onClick={confirmPurge}
            >
              Xóa đã chọn
            </Button>
          </>
        }
      >
        <div className="settings-actions" role="group" aria-label="Chọn trạng thái cần dọn">
          {Object.entries(PURGE_STATUS_LABELS).map(([status, label]) => (
            <label key={status} className="input-field input-field--check">
              <input
                type="checkbox"
                checked={purgeStatuses.includes(status)}
                onChange={(event) =>
                  setPurgeStatuses((prev) =>
                    event.target.checked ? [...prev, status] : prev.filter((s) => s !== status),
                  )
                }
              />
              {label}
            </label>
          ))}
        </div>
        <label className="input-field block-spaced">
          Cũ hơn
          <select
            className="input-field"
            value={purgeOlderThanDays === null ? "all" : String(purgeOlderThanDays)}
            onChange={(event) =>
              setPurgeOlderThanDays(
                event.target.value === "all" ? null : Number(event.target.value),
              )
            }
            aria-label="Giới hạn tuổi của lịch sử bị xóa"
          >
            <option value="all">Toàn bộ thời gian</option>
            <option value="7">7 ngày trước</option>
            <option value="30">30 ngày trước</option>
            <option value="90">90 ngày trước</option>
          </select>
        </label>
      </Dialog>
    </div>
  );
}
