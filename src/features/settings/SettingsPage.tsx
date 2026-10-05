import { useState } from "react";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import type { ReadingDirection, ReaderMode } from "../../domain/reader";
import {
  chooseDownloadRoot,
  clearRegenerableCache,
  getDownloadRoot,
  normalizeIpcError,
  resetDownloadRoot,
  storageStatus,
} from "../../shared/api/desktop";
import type { CacheCleanupDto, DownloadRootDto, StorageStatusDto } from "../../types/backend";
import { formatBytes } from "../../types/downloads";
import { services } from "../../services/provider";
import {
  SETTINGS_LIMITS,
  type AppSettings,
  type OutputFormat,
} from "../../services/settings.service";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";

export default function SettingsPage() {
  const settingsVersion = useDomainVersion("settings");
  const settings = useAsyncData(() => services.settings.get(), [settingsVersion]);

  if (settings.state === "loading") {
    return <Skeleton height="20rem" label="Đang tải cài đặt" />;
  }
  if (settings.state === "error") {
    return (
      <ErrorState
        title="Không tải được cài đặt"
        description={normalizeIpcError(settings.error).message}
      />
    );
  }

  return <SettingsForm initial={settings.data} />;
}

function SettingsForm({ initial }: { initial: AppSettings }) {
  const [draft, setDraft] = useState<AppSettings>(initial);
  const [feedback, setFeedback] = useState<string | null>(null);
  const dirty = JSON.stringify(draft) !== JSON.stringify(initial);

  async function save() {
    setFeedback(null);
    try {
      await services.settings.save(draft);
      bumpDomain("settings");
      setFeedback("Đã lưu.");
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : "Không thể lưu cài đặt.");
    }
  }

  async function reset() {
    try {
      await services.settings.reset();
      setDraft(await services.settings.get());
      bumpDomain("settings");
      setFeedback("Đã khôi phục giá trị mặc định trong database.");
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : "Không thể khôi phục mặc định.");
    }
  }

  return (
    <div className="page-stack">
      <div className="settings-grid">
      <section className="panel" aria-label="Cài đặt reader">
        <h2 className="panel__title">Reader</h2>
        <label className="input-field">
          <span className="input-field__label">Chế độ đọc mặc định</span>
          <select
            className="input"
            value={draft.defaultReaderMode}
            onChange={(event) => setDraft({ ...draft, defaultReaderMode: event.target.value as ReaderMode })}
          >
            <option value="single">Một trang</option>
            <option value="double">Hai trang</option>
            <option value="vertical">Cuộn dọc</option>
          </select>
        </label>
        <label className="input-field">
          <span className="input-field__label">Hướng đọc mặc định</span>
          <select
            className="input"
            value={draft.defaultReadingDirection}
            onChange={(event) => setDraft({ ...draft, defaultReadingDirection: event.target.value as ReadingDirection })}
          >
            <option value="rtl">RTL (phải sang trái)</option>
            <option value="ltr">LTR (trái sang phải)</option>
          </select>
        </label>
      </section>

      <section className="panel" aria-label="Cài đặt downloads">
        <h2 className="panel__title">Downloads</h2>
        <label className="input-field">
          <span className="input-field__label">
            Chương đồng thời ({SETTINGS_LIMITS.chapterConcurrency.min}–{SETTINGS_LIMITS.chapterConcurrency.max})
          </span>
          <input
            className="input"
            type="number"
            min={SETTINGS_LIMITS.chapterConcurrency.min}
            max={SETTINGS_LIMITS.chapterConcurrency.max}
            value={draft.chapterConcurrency}
            onChange={(event) => setDraft({ ...draft, chapterConcurrency: Number(event.target.value) })}
          />
        </label>
        <label className="input-field">
          <span className="input-field__label">
            Ảnh đồng thời ({SETTINGS_LIMITS.imageConcurrency.min}–{SETTINGS_LIMITS.imageConcurrency.max})
          </span>
          <input
            className="input"
            type="number"
            min={SETTINGS_LIMITS.imageConcurrency.min}
            max={SETTINGS_LIMITS.imageConcurrency.max}
            value={draft.imageConcurrency}
            onChange={(event) => setDraft({ ...draft, imageConcurrency: Number(event.target.value) })}
          />
        </label>
        <label className="input-field">
          <span className="input-field__label">Định dạng xuất mặc định</span>
          <select
            className="input"
            value={draft.outputFormat === "pdf" ? "cbz" : draft.outputFormat}
            onChange={(event) => setDraft({ ...draft, outputFormat: event.target.value as OutputFormat })}
          >
            <option value="cbz">CBZ</option>
            <option value="folder">Thư mục ảnh</option>
          </select>
        </label>
      </section>

      <section className="panel" aria-label="Cài đặt Windows">
        <h2 className="panel__title">Windows</h2>
        <label className="input-field input-field--check">
          <input
            type="checkbox"
            checked={draft.notificationsEnabled}
            onChange={(event) => setDraft({ ...draft, notificationsEnabled: event.target.checked })}
          />
          <span>Thông báo khi một chương tải xong hoặc thất bại (tổng hợp theo chương, không theo ảnh)</span>
        </label>
        <label className="input-field input-field--check">
          <input
            type="checkbox"
            checked={draft.closeToTray}
            onChange={(event) => setDraft({ ...draft, closeToTray: event.target.checked })}
          />
          <span>
            Nút X thu nhỏ xuống system tray thay vì thoát (tray có Show/Hide · Open Downloads · Quit;
            Quit vẫn dừng worker theo lifecycle — không hứa tải tiếp sau khi process thoát)
          </span>
        </label>
        <label className="input-field input-field--check">
          <input
            type="checkbox"
            checked={draft.watchEnabled}
            onChange={(event) => setDraft({ ...draft, watchEnabled: event.target.checked })}
          />
          <span>
            Tự kiểm tra chương mới cho truyện đã lưu (mặc định toàn cục; tắt/bật từng truyện trong
            trang Library). App chỉ kiểm tra khi đang chạy — chu kỳ ~6 giờ, Pace theo nguồn, và chỉ
            với truyện đã bật theo dõi.
          </span>
        </label>
      </section>

      <StoragePanel />
      </div>

      <div className="settings-actions">
        <Button variant="primary" onClick={() => void save()} disabled={!dirty}>
          {dirty ? "Lưu thay đổi" : "Đã lưu — không có thay đổi"}
        </Button>
        <Button variant="secondary" onClick={() => void reset()}>
          Khôi phục mặc định
        </Button>
        <Button to="/settings/sources" variant="ghost">
          Quản lý nguồn (Sources)
        </Button>
      </div>
      {feedback && (
        <p className="muted" role="status">
          {feedback}
        </p>
      )}
    </div>
  );
}

function StoragePanel() {
  const [nonce, setNonce] = useState(0);
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [result, setResult] = useState<CacheCleanupDto | null>(null);
  const [feedbackMessage, setFeedbackMessage] = useState<string | null>(null);
  const [rootBusy, setRootBusy] = useState(false);
  const [rootError, setRootError] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const root = useAsyncData<DownloadRootDto | null>(async () => {
    try {
      return await getDownloadRoot();
    } catch {
      return null;
    }
  }, [nonce]);
  const status = useAsyncData<StorageStatusDto | null>(async () => {
    try {
      return await storageStatus();
    } catch {
      return null;
    }
  }, [nonce]);

  async function clearCache() {
    // Two-step inline confirm (native window.confirm is unreliable in
    // embedded webviews and off-design-system).
    setConfirming(false);
    setBusy(true);
    setResult(null);
    try {
      const outcome = await clearRegenerableCache();
      setResult(outcome);
      bumpDomain("downloads");
      setNonce((value) => value + 1);
    } catch (error) {
      setFeedbackMessage(normalizeIpcError(error).message);
    } finally {
      setBusy(false);
    }
  }

  async function runRootAction(action: () => Promise<DownloadRootDto | null>) {
    setRootBusy(true);
    setRootError(null);
    try {
      const outcome = await action(); // null = user cancelled the dialog
      if (outcome) {
        bumpDomain("downloads");
        bumpDomain("library");
        bumpDomain("home");
        setNonce((value) => value + 1);
      }
    } catch (error) {
      setRootError(normalizeIpcError(error).message);
    } finally {
      setRootBusy(false);
      setConfirmReset(false);
    }
  }

  return (
    <section className="panel" aria-label="Lưu trữ và cache">
      <h2 className="panel__title">Lưu trữ</h2>
      {status.state === "loading" && <Skeleton height="4rem" label="Đang tính dung lượng" />}
      {status.state === "error" && (
        <p className="queue-error" role="alert">
          {normalizeIpcError(status.error).message}
        </p>
      )}
      {status.state === "success" && status.data && (
        <div className="storage-status">
          {!status.data.rootAvailable && (
            <p className="queue-error" role="alert">
              Không đọc được thư mục library (ổ đĩa bị ngắt?) — việc đọc/tải offline sẽ báo lỗi cho tới
              khi root khả dụng lại.
            </p>
          )}
          {root.state === "success" && root.data && (
            <p className="panel__description">
              Nơi lưu truyện đã tải: <strong>{root.data.label}</strong>
              {root.data.isCustom ? " (thư mục tùy chọn)" : ""}
            </p>
          )}
          {root.state === "success" && root.data && !root.data.linkOk && (
            <p className="queue-error" role="alert">
              Link tới thư mục tùy chọn đang hỏng (đổi tên hay di chuyển thư mục `library`?). Truyện
              KHÔNG bị mất — bấm &quot;Chọn thư mục lưu…&quot; rồi chọn đúng thư mục ĐANG chứa thư mục
              <strong> library </strong> để tái liên kết.
            </p>
          )}
          {rootError && (
            <p className="queue-error" role="alert">
              Không đổi được nơi lưu — {rootError}
            </p>
          )}
          <div className="settings-actions">
            <Button
              variant="secondary"
              disabled={rootBusy}
              onClick={() => void runRootAction(() => chooseDownloadRoot())}
            >
              {rootBusy ? "Đang chuyển thư viện…" : "Chọn thư mục lưu…"}
            </Button>
            {root.state === "success" && root.data?.isCustom ? (
              confirmReset ? (
                <>
                  <Button
                    variant="primary"
                    disabled={rootBusy}
                    onClick={() => void runRootAction(() => resetDownloadRoot())}
                  >
                    Xác nhận: chuyển về mặc định
                  </Button>
                  <Button variant="ghost" onClick={() => setConfirmReset(false)}>
                    Hủy
                  </Button>
                </>
              ) : (
                <Button variant="ghost" disabled={rootBusy} onClick={() => setConfirmReset(true)}>
                  Về nơi lưu mặc định
                </Button>
              )
            ) : null}
            {confirming ? (
              <>
                <Button variant="primary" onClick={() => void clearCache()} disabled={busy}>
                  Xác nhận dọn cache tạm
                </Button>
                <Button variant="ghost" onClick={() => setConfirming(false)}>
                  Hủy
                </Button>
              </>
            ) : (
              <Button
                variant="secondary"
                onClick={() => setConfirming(true)}
                disabled={busy}
                aria-label="Dọn cache tái tạo được"
              >
                {busy ? "Đang dọn…" : "Dọn cache tái tạo được"}
              </Button>
            )}
            <Button variant="ghost" onClick={() => setNonce((value) => value + 1)}>
              Tính lại
            </Button>
          </div>
          {confirming && (
            <p className="muted" role="status">
              Sẽ xóa staging của các tác vụ đã kết thúc + cache covers. Truyện đã tải,
              checkpoint đang chạy và tiến độ đọc KHÔNG bị ảnh hưởng.
            </p>
          )}
          {result && (
            <p className="muted" role="status">
              Đã xóa {result.removedDirs} thư mục tạm ({formatBytes(result.freedBytes)}) · giữ{" "}
              {result.keptActiveDirs} thư mục của tác vụ đang chạy.
            </p>
          )}
          {feedbackMessage && (
            <p className="queue-error" role="alert">
              {feedbackMessage}
            </p>
          )}
        </div>
      )}
      {status.state === "success" && !status.data && (
        <p className="muted">Không đọc được trạng thái lưu trữ — thử lại sau.</p>
      )}
    </section>
  );
}

