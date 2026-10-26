import { useState } from "react";
import { Outlet } from "react-router-dom";
import AppEventBridge from "../../app/AppEventBridge";
import ErrorBoundary from "../../app/ErrorBoundary";
import CommandPalette from "../../features/global-search/CommandPalette";
import ToastViewport from "../ui/Toast";
import PageContainer from "./PageContainer";
import ShortcutsDialog from "./ShortcutsDialog";
import Sidebar from "./Sidebar";
import Topbar from "./Topbar";
import WindowTitlebar from "./WindowTitlebar";

export default function AppShell() {
  const [collapsed, setCollapsed] = useState(false);

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
