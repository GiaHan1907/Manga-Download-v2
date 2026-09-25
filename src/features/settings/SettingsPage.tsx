import { useState } from "react";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import type { ReadingDirection, ReaderMode } from "../../domain/reader";
import {
  chooseDownloadRoot,
  clearRegenerableCache,
  getAppInfo,
  getDownloadRoot,
  normalizeIpcError,
  resetDownloadRoot,
  storageStatus,
} from "../../shared/api/desktop";
import type { AppInfo } from "../../shared/api/contracts";
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
  const appInfo = useAsyncData<AppInfo | null>(async () => {
    try {
      return await getAppInfo();
    } catch {
      return null;
    }
  }, []);

  if (settings.state === "loading") {
    return <Skeleton height="20rem" label="Đang tải cài đặt" />;
  }
  if (settings.state === "error") {
    return (
      <ErrorState
        title="Không tải được cài đặt"
        description={`${normalizeIpcError(settings.error).message} — đường dẫn này là SQLite thật, không có mock.`}
      />
    );
  }

  return <SettingsForm initial={settings.data} appInfo={appInfo} />;
}

function SettingsForm({ initial, appInfo }: { initial: AppSettings; appInfo: AsyncAppInfo }) {
  const [draft, setDraft] = useState<AppSettings>(initial);
  const [feedback, setFeedback] = useState<string | null>(null);
  const dirty = JSON.stringify(draft) !== JSON.stringify(initial);

  async function save() {
    setFeedback(null);
    try {
      await services.settings.save(draft);
      bumpDomain("settings");
      setFeedback("Đã lưu vào SQLite — giá trị giữ nguyên qua khởi động lại.");
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
      <section className="panel" aria-label="Cài đặt chung">
        <h2 className="panel__title">General</h2>
        <p className="panel__description">
          Chủ đề: Dark Purple — hệ token duy nhất của app (Phase 02). Light/Auto chưa được thiết kế
          accessibility nên KHÔNG có trong settings.
        </p>
      </section>

      <section className="panel" aria-label="Cài đặt reader">
        <h2 className="panel__title">Reader</h2>
        <p className="panel__description">
          Áp dụng cho lần MỞ chương kế tiếp (reader đang mở giữ lựa chọn phiên của nó).
        </p>
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
        <p className="panel__description">
          Engine Rust đọc các giá trị này ở mỗi nhịp lập lịch — đổi chỉ ảnh hưởng job mới, không
          phá checkpoint đang chạy.
        </p>
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
            <option value="cbz">CBZ (xuất thật — đã kiểm chứng ZIP STORE)</option>
            <option value="folder">Thư mục ảnh (xuất thật)</option>
          </select>
        </label>
        <p className="muted">
          PDF không có trong danh sách vì engine Phase 06 không xuất PDF — chọn sẽ bị server từ chối.
        </p>
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
        <p className="muted">Cần lưu để có hiệu lực.</p>
      </section>

      <StoragePanel />

      <section className="panel" aria-label="Về ứng dụng">
        <h2 className="panel__title">About</h2>
        {appInfo.state === "success" && appInfo.data ? (
          <p className="panel__description">
            {appInfo.data.name} · phiên bản thật {appInfo.data.version} (IPC Phase 01) · {appInfo.data.platform}
          </p>
        ) : (
          <p className="panel__description">Không đọc được thông tin app qua IPC — kiểm tra Console.</p>
        )}
        <p className="panel__description">
          Mọi cài đặt ở trang này được validate trong Rust và lưu vào SQLite (Phase 04). Không có
          control nào ở đây là mock.
        </p>
        <div className="chip-row">
          <StatusBadge tone="success">Persisted — SQLite</StatusBadge>
        </div>
      </section>
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
          <p className="panel__description">
            Thư viện: <strong>{formatBytes(status.data.libraryBytes)}</strong> · Cache/staging:{" "}
            <strong>{formatBytes(status.data.cacheBytes)}</strong> · {status.data.stagingDirs} thư mục
            tạm · {status.data.activeTasks} tác vụ đang chạy. Số liệu là{" "}
            <em>xấp xỉ</em> (theo DB + walk thư mục quản lý của app).
          </p>
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
              Link tới thư mục tùy chọn đang hỏng (ngắt ổ đĩa / xóa thư mục?). Truyện KHÔNG bị mất — bấm
              &quot;Chọn thư mục lưu…&quot; và trỏ lại đúng thư mục cũ để tái liên kết.
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
      <p className="muted">
        Policy: không đổi được thư mục library ở Phase 08 (đảm bảo file cũ không bị mồ côi); không có
        nút "mở thư mục" vì shell capability chưa được cấp.
      </p>
    </section>
  );
}

type AsyncAppInfo = { state: "loading" } | { state: "error"; error: unknown } | { state: "success"; data: AppInfo | null };
