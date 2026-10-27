import { useEffect, useState } from "react";
import Dialog from "../ui/Dialog";
import { isTypingTarget } from "../../shared/api/errors";

/// Fired by the command palette ("Phím tắt") — same shape as the palette's own
/// open event, so neither component needs state lifted into the shell.
export const OPEN_SHORTCUTS_EVENT = "app-open-shortcuts";

/// Every shortcut that actually exists in the code: the global palette, the
/// dialog convention, and the reader's own key map (ReadPage). Keeping this
/// list beside the reader would hide it from the rest of the app; keeping it
/// here means it must be updated when a binding moves.
const GLOBAL_KEYS: [string, string][] = [
  ["Ctrl + K", "Tìm kiếm và lệnh nhanh"],
  ["?", "Mở bảng phím tắt này"],
  ["Esc", "Đóng hộp thoại đang mở"],
];

const READER_KEYS: [string, string][] = [
  ["← / →", "Trang trước / sau (theo hướng đọc đã chọn)"],
  ["Space hoặc PageDown", "Trang sau"],
  ["PageUp", "Trang trước"],
  ["Home / End", "Trang đầu / trang cuối"],
  ["+ / −", "Phóng to / thu nhỏ"],
  ["0", "Về mức phóng to gốc"],
  ["M", "Đổi chế độ đọc (một trang → hai trang → cuộn dọc)"],
  ["T", "Ẩn/hiện thanh công cụ"],
  ["F hoặc F11", "Bật/tắt toàn màn hình"],
  ["Esc", "Thoát toàn màn hình, hoặc về trang truyện"],
];

export default function ShortcutsDialog() {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      // "?" is Shift+/ — accept the raw key only, and never while the user is
      // typing a query (the palette input, the library search box…).
      if (event.key !== "?" || event.ctrlKey || event.metaKey || event.altKey) {
        return;
      }
      if (isTypingTarget(event.target)) {
        return;
      }
      event.preventDefault();
      setOpen(true);
    }
    const onOpen = () => setOpen(true);
    window.addEventListener("keydown", onKey);
    window.addEventListener(OPEN_SHORTCUTS_EVENT, onOpen);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener(OPEN_SHORTCUTS_EVENT, onOpen);
    };
  }, []);

  return (
    <Dialog
      open={open}
      title="Phím tắt"
      description="Phím tắt dùng được bất cứ lúc nào trong app; nhóm dưới chỉ có tác dụng khi đang đọc chương."
      onClose={() => setOpen(false)}
    >
      <h3 className="shortcut-group">Toàn app</h3>
      <dl className="shortcut-list">
        {GLOBAL_KEYS.map(([keys, action]) => (
          <div key={keys} className="shortcut-list__row">
            <dt>
              <kbd className="shortcut-key">{keys}</kbd>
            </dt>
            <dd>{action}</dd>
          </div>
        ))}
      </dl>
      <h3 className="shortcut-group">Trình đọc</h3>
      <dl className="shortcut-list">
        {READER_KEYS.map(([keys, action]) => (
          <div key={keys} className="shortcut-list__row">
            <dt>
              <kbd className="shortcut-key">{keys}</kbd>
            </dt>
            <dd>{action}</dd>
          </div>
        ))}
      </dl>
    </Dialog>
  );
}