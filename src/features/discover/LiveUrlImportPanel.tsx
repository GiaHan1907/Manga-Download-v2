import { useState } from "react";
import { useNavigate } from "react-router-dom";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Input from "../../components/ui/Input";
import { localServices } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";

export default function LiveUrlImportPanel() {
  const navigate = useNavigate();
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  async function submit() {
    const trimmed = url.trim();
    if (!trimmed || busy) {
      return;
    }
    setBusy(true);
    setFailure(null);
    try {
      const detail = await localServices.source.resolveUrl(trimmed);
      navigate(
        `/remote/${encodeURIComponent(detail.sourceId)}/${encodeURIComponent(detail.sourceMangaId)}`,
      );
    } catch (error: unknown) {
      const normalized = normalizeIpcError(error);
      setFailure(`${normalized.message} (mã: ${normalized.code})`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page-stack">
      <Input
        label="Import URL manga"
        type="url"
        placeholder="https://<domain>/manga/<slug> — thuộc nguồn đã đăng ký"
        value={url}
        onChange={(event) => setUrl(event.target.value)}
        hint="Chỉ nhận HTTPS, host đúng khớp danh sách nguồn đã đăng ký; app không fetch URL lạ để đoán."
      />
      <div className="settings-actions">
        <Button variant="primary" onClick={() => void submit()} disabled={busy || url.trim() === ""}>
          {busy ? "Đang kiểm tra..." : "Kiểm tra URL"}
        </Button>
      </div>
      {failure && <ErrorState title="Không nhận dạng được URL" description={failure} />}
      <p className="muted">
        URL hợp lệ sẽ mở trang xem trước từ xa. Việc dán URL không tự lưu vào Library và không tải bất kỳ file nào.
      </p>
    </div>
  );
}
