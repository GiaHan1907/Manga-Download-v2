import { useState } from "react";
import Button from "../../components/ui/Button";
import { exportLibraryBackup, importLibraryBackup, normalizeIpcError } from "../../shared/api/desktop";
import { bumpDomain } from "../../shared/hooks/useInvalidation";
import type { BackupSummaryDto } from "../../types/backend";

function describe(summary: BackupSummaryDto, past: "exported" | "restored"): string {
  const parts = [
    `${summary.mangas} truyện`,
    summary.favorites > 0 ? `${summary.favorites} yêu thích` : null,
    summary.watched > 0 ? `${summary.watched} đang theo dõi` : null,
    summary.progress > 0 ? `${summary.progress} vị trí đọc` : null,
    summary.skipped > 0 ? `bỏ qua ${summary.skipped} mục không có trong thư viện` : null,
  ].filter(Boolean);
  return past === "exported"
    ? `Đã lưu ${parts.join(", ")} vào file sao lưu.`
    : `Đã khôi phục ${parts.join(", ")}.`;
}

/// Metadata only, on purpose: the CBZ/PDF files live in the library folder and
/// are the user's to copy with any file manager. What is painful to lose is the
/// part only this app knows — which stories are saved, favorites, watch flags,
/// reading position.
export default function BackupPanel() {
  const [busy, setBusy] = useState<"export" | "import" | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);

  async function run(kind: "export" | "import") {
    setBusy(kind);
    setFeedback(null);
    try {
      const summary =
        kind === "export" ? await exportLibraryBackup() : await importLibraryBackup();
      if (!summary) {
        setFeedback("Đã huỷ — không có file nào được ghi hay đọc.");
        return;
      }
      if (kind === "import") {
        // The import can touch favorites, watch flags and progress.
        bumpDomain("favorites");
        bumpDomain("library");
        bumpDomain("home");
      }
      setFeedback(describe(summary, kind === "export" ? "exported" : "restored"));
    } catch (error) {
      setFeedback(normalizeIpcError(error).message);
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="panel" aria-label="Sao lưu và khôi phục">
      <h2 className="panel__title">Sao lưu &amp; khôi phục</h2>
      <p className="panel__description">
        File sao lưu chỉ chứa metadata: truyện đã lưu, yêu thích, theo dõi chương mới và vị trí đọc.
        File truyện (CBZ/PDF) nằm trong thư mục thư viện — sao chép thư mục đó bằng trình quản lý
        file nếu muốn mang cả nội dung. Khôi phục chỉ bổ sung: không xoá yêu thích, không tắt theo
        dõi, không lùi vị trí đọc.
      </p>
      <div className="settings-actions">
        <Button variant="secondary" disabled={busy !== null} onClick={() => void run("export")}>
          {busy === "export" ? "Đang ghi file…" : "Xuất sao lưu"}
        </Button>
        <Button variant="secondary" disabled={busy !== null} onClick={() => void run("import")}>
          {busy === "import" ? "Đang khôi phục…" : "Khôi phục từ file"}
        </Button>
      </div>
      {feedback && (
        <p className="muted" role="status">
          {feedback}
        </p>
      )}
    </section>
  );
}
