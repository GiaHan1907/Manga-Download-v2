import { Search, Settings } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";
import { resolvePageTitle } from "../../app/routes";
import Button from "../ui/Button";
import { OPEN_PALETTE_EVENT } from "../../features/global-search/CommandPalette";

export default function Topbar() {
  const location = useLocation();
  const navigate = useNavigate();
  const pageTitle = resolvePageTitle(location.pathname);

  return (
    <header className="topbar">
      <div>
        <h1 className="topbar__title">{pageTitle}</h1>
      </div>
      <div className="topbar__actions">
        <Button
          variant="secondary"
          onClick={() => window.dispatchEvent(new Event(OPEN_PALETTE_EVENT))}
          aria-label="Tìm kiếm và lệnh nhanh (Ctrl+K)"
        >
          <Search size={18} aria-hidden="true" />
          Tìm kiếm · Ctrl+K
        </Button>
        <Button variant="ghost" iconOnly aria-label="Mở Settings" onClick={() => navigate("/settings")}>
          <Settings size={18} aria-hidden="true" />
        </Button>
      </div>
    </header>
  );
}
