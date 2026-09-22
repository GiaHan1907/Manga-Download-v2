import { useState } from "react";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import type { ReadingDirection, ReaderMode } from "../../domain/reader";
import { getAppInfo } from "../../shared/api/desktop";
import type { AppInfo } from "../../shared/api/contracts";
import { services } from "../../services/provider";
import { SETTINGS_LIMITS, type AppSettings, type OutputFormat } from "../../services/settings.service";
import { useAsyncData } from "../../shared/hooks/useAsyncData";

export default function SettingsPage() {
  const settings = useAsyncData(() => services.settings.get(), []);
  const appInfo = useAsyncData<AppInfo | null>(async () => {
    try {
      return await getAppInfo();
    } catch {
      return null;
    }
  }, []);

  if (settings.state === "loading") {
    return <Skeleton height="20rem" label="Đang tải cài đặt demo" />;
  }
  if (settings.state === "error") {
    return <ErrorState title="Không tải được cài đặt demo" description="Mock service lỗi. Reload để thử lại." />;
  }

  return <SettingsForm initial={settings.data} appInfo={appInfo} />;
}

function SettingsForm({ initial, appInfo }: { initial: AppSettings; appInfo: AsyncAppInfo }) {
  const [draft, setDraft] = useState<AppSettings>(initial);
  const [feedback, setFeedback] = useState<string | null>(null);

  async function save() {
    setFeedback(null);
    try {
      await services.settings.save(draft);
      setFeedback("Đã lưu vào SQLite — giá trị giữ nguyên qua khởi động lại.");
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : "Không thể lưu cài đặt.");
    }
  }

  async function reset() {
    await services.settings.reset();
    setDraft(await services.settings.get());
    setFeedback("Đã khôi phục giá trị mặc định trong database.");
  }

  return (
    <div className="page-stack settings-grid">
      <section className="panel" aria-label="Cài đặt chung">
        <h2 className="panel__title">General</h2>
        <p className="panel__description">Chủ đề: Dark Purple — mặc định của Phase 02, chưa có light mode trong prototype.</p>
      </section>

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

      <section className="panel" aria-label="Cài đặt downloads (demo)">
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
          <span className="input-field__label">Định dạng đầu ra</span>
          <select
            className="input"
            value={draft.outputFormat}
            onChange={(event) => setDraft({ ...draft, outputFormat: event.target.value as OutputFormat })}
          >
            <option value="cbz">CBZ (planned — chưa xuất thật)</option>
            <option value="folder">Image Folder (planned — chưa ghi thật)</option>
            <option value="pdf">PDF (planned — chưa xuất thật)</option>
          </select>
        </label>
        <p className="muted">Lưu trữ: Not connected in UI prototype — không có duyệt thư mục hay grant quyền.</p>
      </section>

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
          Cài đặt được lưu thật vào SQLite (Phase 04). Các giá trị export/URL worker chỉ là sở thích — engine thật thuộc Phase 05–07.
        </p>
        <div className="chip-row">
          <StatusBadge tone="success">Persisted — SQLite</StatusBadge>
          <StatusBadge tone="neutral">Session-scoped</StatusBadge>
        </div>
      </section>

      <div className="settings-actions">
        <Button variant="primary" onClick={() => void save()}>Lưu (mock)</Button>
        <Button variant="secondary" onClick={() => void reset()}>Khôi phục mặc định</Button>
      </div>
      {feedback && (
        <p className="muted" role="status">
          {feedback}
        </p>
      )}
    </div>
  );
}

type AsyncAppInfo = { state: "loading" } | { state: "error"; error: unknown } | { state: "success"; data: AppInfo | null };
