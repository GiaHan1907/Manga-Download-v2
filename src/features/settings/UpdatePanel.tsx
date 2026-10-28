import { useState } from "react";
import Button from "../../components/ui/Button";
import { checkForUpdate, installUpdate, normalizeIpcError } from "../../shared/api/desktop";
import type { UpdateCheckDto } from "../../types/backend";

/// Checks the release endpoint configured in tauri.conf.json and, when a newer
/// version exists, downloads and installs it. The signature is verified by the
/// updater plugin against the embedded public key before anything runs.
export default function UpdatePanel() {
  const [busy, setBusy] = useState<"check" | "install" | null>(null);
  const [result, setResult] = useState<UpdateCheckDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function check() {
    setBusy("check");
    setError(null);
    try {
      setResult(await checkForUpdate());
    } catch (failure) {
      setError(normalizeIpcError(failure).message);
    } finally {
      setBusy(null);
    }
  }

  async function install() {
    setBusy("install");
    setError(null);
    try {
      // On success the app restarts into the new build, so this promise may
      // never resolve from the page's point of view.
      await installUpdate();
    } catch (failure) {
      setError(normalizeIpcError(failure).message);
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="panel" aria-label="Cập nhật ứng dụng">
      <h2 className="panel__title">Cập nhật</h2>
      <p className="panel__description">
        Bản mới được tải từ GitHub Releases và kiểm tra chữ ký trước khi cài.
        {result ? ` Bản đang chạy: ${result.current}.` : ""}
      </p>
      <div className="settings-actions">
        <Button variant="secondary" disabled={busy !== null} onClick={() => void check()}>
          {busy === "check" ? "Đang kiểm tra…" : "Kiểm tra cập nhật"}
        </Button>
        {result?.available && (
          <Button variant="primary" disabled={busy !== null} onClick={() => void install()}>
            {busy === "install" ? "Đang tải & cài…" : `Cài v${result.version} & khởi động lại`}
          </Button>
        )}
      </div>
      {result && !result.available && (
        <p className="muted" role="status">
          Bạn đang dùng bản mới nhất ({result.current}).
        </p>
      )}
      {result?.available && result.notes && (
        <p className="muted" role="status">
          {result.notes}
        </p>
      )}
      {error && (
        <p className="queue-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
