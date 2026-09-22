import { Minus, Square, X } from "lucide-react";
import { windowService } from "../../services/windowService";

export default function WindowTitlebar() {
  return (
    <header className="window-titlebar">
      <div className="window-titlebar__drag-region" data-tauri-drag-region onDoubleClick={() => void windowService.toggleMaximize()}>
        <div className="window-titlebar__icon" aria-hidden="true">M</div>
        <div className="window-titlebar__text" data-tauri-drag-region>
          <div className="window-titlebar__name" data-tauri-drag-region>
            Manga Downloader 1989
          </div>
          <div className="window-titlebar__subtitle" data-tauri-drag-region>
            Design System & UI Shell
          </div>
        </div>
      </div>
      <div className="window-titlebar__controls" role="group" aria-label="Điều khiển cửa sổ">
        <button className="window-titlebar__button" type="button" aria-label="Thu nhỏ cửa sổ" onClick={() => void windowService.minimize()}>
          <Minus size={16} aria-hidden="true" />
        </button>
        <button className="window-titlebar__button" type="button" aria-label="Phóng to hoặc khôi phục cửa sổ" onClick={() => void windowService.toggleMaximize()}>
          <Square size={14} aria-hidden="true" />
        </button>
        <button className="window-titlebar__button window-titlebar__button--danger" type="button" aria-label="Đóng cửa sổ" onClick={() => void windowService.close()}>
          <X size={16} aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}
