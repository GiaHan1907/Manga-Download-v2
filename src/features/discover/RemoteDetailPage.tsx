import { useState } from "react";
import { useParams } from "react-router-dom";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import Tooltip from "../../components/ui/Tooltip";
import { localServices } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain } from "../../shared/hooks/useInvalidation";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";
import type { SaveResultDto } from "../../types/source";

type SaveState =
  | { status: "idle" }
  | { status: "saving" }
  | { status: "done"; result: SaveResultDto }
  | { status: "error"; message: string };

const STATUS_LABELS: Record<string, string> = {
  ongoing: "Đang tiến hành",
  completed: "Hoàn thành",
  unknown: "Không rõ",
};

export default function RemoteDetailPage() {
  const { sourceId = "", sourceMangaId = "" } = useParams();
  const [refresh, setRefresh] = useState(0);
  const [saveState, setSaveState] = useState<SaveState>({ status: "idle" });

  const detail = useAsyncData(
    () => localServices.source.remoteDetail({ sourceId, sourceMangaId }),
    [sourceId, sourceMangaId, refresh],
  );

  const coverUrl = useCoverUrl(
    sourceId,
    sourceMangaId,
    detail.state === "success" ? detail.data.coverKey : null,
  );

  async function saveToLibrary() {
    setSaveState({ status: "saving" });
    try {
      const result = await localServices.source.save({ sourceId, sourceMangaId });
      setSaveState({ status: "done", result });
      // Cross-screen: Library/Home now include the newly saved manga.
      bumpDomain("library");
      bumpDomain("home");
      setRefresh((n) => n + 1);
    } catch (error: unknown) {
      setSaveState({ status: "error", message: normalizeIpcError(error).message });
    }
  }

  if (detail.state === "loading") {
    return (
      <div className="page-stack" aria-busy="true">
        <Skeleton height="10rem" />
        <Skeleton height="16rem" />
      </div>
    );
  }
  if (detail.state === "error") {
    const error = normalizeIpcError(detail.error);
    return (
      <ErrorState
        title="Không xem trước được bản ghi từ xa"
        description={`${error.message} (mã: ${error.code})`}
        action={
          <Button onClick={() => setRefresh((n) => n + 1)} variant="primary">
            Thử lại
          </Button>
        }
      />
    );
  }

  const manga = detail.data;
  const savedId = saveState.status === "done" ? saveState.result.mangaId : manga.savedMangaId;
  const alreadySaved = savedId !== null;

  return (
    <div className="page-stack">
      <div className="remote-preview__row">
        {coverUrl && (
          <img
            src={coverUrl}
            alt={`Ảnh bìa ${manga.title}`}
            width={160}
            className="remote-preview__cover"
          />
        )}
        <div>
          <h2>{manga.title}</h2>
          <p className="muted">Nguồn: {manga.sourceDisplayName}</p>
        </div>
      </div>
        <div className="chip-row">
          <StatusBadge>{STATUS_LABELS[manga.status] ?? manga.status}</StatusBadge>
          <StatusBadge tone="warning">Xem trước từ xa</StatusBadge>
          {alreadySaved ? (
            <StatusBadge tone="success">Đã lưu metadata</StatusBadge>
          ) : (
            <StatusBadge tone="neutral">Chưa vào Library</StatusBadge>
          )}
          <StatusBadge tone="neutral">Chưa tải — Phase 06</StatusBadge>
        </div>

      {manga.genres.length > 0 && (
        <div className="chip-row" aria-label="Thể loại">
          {manga.genres.map((genre) => (
            <StatusBadge key={genre} tone="neutral">
              {genre}
            </StatusBadge>
          ))}
        </div>
      )}
      {manga.description && <p>{manga.description}</p>}
      {manga.canonicalUrl && (
        <p className="muted">URL gốc: {manga.canonicalUrl}</p>
      )}

      {saveState.status === "done" && (
        <p role="status" className="muted">
          {saveState.result.updated
            ? `Đã cập nhật metadata — ${saveState.result.savedChapterCount} chương (id Library: ${saveState.result.mangaId}).`
            : `Đã lưu vào Library (chỉ metadata) — ${saveState.result.savedChapterCount} chương.`}{" "}
          <Button to="/library" variant="secondary">
            Mở Library
          </Button>
        </p>
      )}
      {saveState.status === "error" && (
        <ErrorState title="Không lưu được" description={saveState.message} />
      )}

      <div className="settings-actions">
        <Button
          variant="primary"
          onClick={() => void saveToLibrary()}
          disabled={saveState.status === "saving"}
        >
          {saveState.status === "saving"
            ? "Đang lưu..."
            : alreadySaved
              ? "Làm mới metadata & chương"
              : "Lưu vào Library"}
        </Button>
        <Tooltip
          content="Lưu truyện vào Library trước — sau đó chọn chương để tải ngay trong trang chi tiết Library."
          focusableWhenDisabled
        >
          <Button disabled aria-label="Tải chương (chỉ khả dụng với truyện đã lưu)">
            Tải chương
          </Button>
        </Tooltip>
      </div>

      <section aria-label="Danh sách chương từ xa">
        <h3>Chương ({manga.chapters.length})</h3>
        {manga.chapters.length === 0 ? (
          <p className="muted">Nguồn không trả về chương nào cho bản ghi này.</p>
        ) : (
          <ol className="chapter-list">
            {manga.chapters.map((chapter) => (
              <li key={chapter.sourceChapterId}>
                {chapter.title}
                {chapter.number !== null ? ` — Chương ${chapter.number}` : ""}
              </li>
            ))}
          </ol>
        )}
      </section>
    </div>
  );
}
