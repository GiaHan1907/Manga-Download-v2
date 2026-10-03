import { useState } from "react";
import Button from "../../components/ui/Button";
import Dialog from "../../components/ui/Dialog";
import { localServices } from "../../services/provider";
import type { RemoveMangaDto } from "../../types/library";

/// "Xóa truyện khỏi Thư viện" — the erase path shared by the Library card and
/// the local detail page. One dialog, two file modes: keep the downloaded
/// .cbz files on disk, or remove them with the records. Everything the
/// database holds for the manga (chapters, tasks, progress, favorites,
/// alerts, history, genres) is erased server-side in one transaction.
export default function MangaDeleteDialog({
  mangaId,
  mangaTitle,
  downloadedChapters,
  onClose,
  onDeleted,
}: {
  mangaId: string;
  mangaTitle: string;
  downloadedChapters: number;
  onClose: () => void;
  onDeleted: (outcome: RemoveMangaDto) => void;
}) {
  const [deleteFiles, setDeleteFiles] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function confirm() {
    setBusy(true);
    setError(null);
    try {
      const outcome = await localServices.libraryV2.removeManga({ mangaId, deleteFiles });
      onDeleted(outcome);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Không xóa được truyện.");
      setBusy(false);
    }
  }

  return (
    <Dialog
      open
      title="Xóa truyện khỏi Thư viện"
      description={`Sẽ gỡ "${mangaTitle}" khỏi Thư viện: danh mục chương, tiến độ đọc, yêu thích, alerts và lịch sử của truyện. Hành mục khác trong Thư viện không bị ảnh hưởng.`}
      onClose={busy ? () => undefined : onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            Hủy
          </Button>
          <Button variant="primary" onClick={() => void confirm()} disabled={busy}>
            {busy ? "Đang xóa…" : "Xóa truyện"}
          </Button>
        </>
      }
    >
      <div className="settings-actions" role="radiogroup" aria-label="Cách xử lý file đã tải">
        <label className="input-field input-field--check block-spaced">
          <input
            type="radio"
            name="erase-files-mode"
            checked={!deleteFiles}
            onChange={() => setDeleteFiles(false)}
          />
          Giữ file đã tải trong thư mục ({downloadedChapters} chương) — chỉ gỡ khỏi Thư viện.
        </label>
        <label className="input-field input-field--check block-spaced">
          <input
            type="radio"
            name="erase-files-mode"
            checked={deleteFiles}
            onChange={() => setDeleteFiles(true)}
          />
          Xóa cả {downloadedChapters} file đã tải ({downloadedChapters === 0 ? "không có file" : "không thể hoàn tác"}).
        </label>
      </div>
      {error && (
        <p className="queue-error" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}
