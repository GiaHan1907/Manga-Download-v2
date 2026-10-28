import { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import AppEventBridge from "../../app/AppEventBridge";
import ErrorBoundary from "../../app/ErrorBoundary";
import CommandPalette from "../../features/global-search/CommandPalette";
import ToastViewport from "../ui/Toast";
import { pushToast } from "../ui/toastStore";
import { checkForUpdate } from "../../shared/api/desktop";
import PageContainer from "./PageContainer";
import ShortcutsDialog from "./ShortcutsDialog";
import Sidebar from "./Sidebar";
import Topbar from "./Topbar";
import WindowTitlebar from "./WindowTitlebar";

export default function AppShell() {
  const [collapsed, setCollapsed] = useState(false);

  // One quiet check after first paint: never blocks startup, never a modal.
  // The updater is a no-op in dev/unsigned builds (the call just fails).
  useEffect(() => {
    const timer = window.setTimeout(() => {
      void checkForUpdate()
        .then((result) => {
          if (result?.available) {
            pushToast(`Có bản cập nhật v${result.version} — mở Cài đặt để cài.`);
          }
        })
        .catch(() => undefined);
    }, 10_000);
    return () => window.clearTimeout(timer);
  }, []);

  return (
    <div className={`app-shell${collapsed ? " app-shell--collapsed" : ""}`}>
      <AppEventBridge />
      <WindowTitlebar />
      <div className="app-shell__body">
        <Sidebar collapsed={collapsed} onToggleCollapsed={() => setCollapsed((value) => !value)} />
        <section className="app-shell__main" aria-label="Nội dung ứng dụng">
          <Topbar />
          <PageContainer>
            <ErrorBoundary>
              <Outlet />
            </ErrorBoundary>
          </PageContainer>
        </section>
      </div>
      <CommandPalette />
      <ShortcutsDialog />
      <ToastViewport />
    </div>
  );
}
