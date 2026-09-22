import { Search, Settings } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";
import { resolvePageTitle } from "../../app/routes";
import Button from "../ui/Button";
import StatusBadge from "../ui/StatusBadge";

export default function Topbar() {
  const location = useLocation();
  const navigate = useNavigate();
  const pageTitle = resolvePageTitle(location.pathname);

  return (
    <header className="topbar">
      <div>
        <p className="muted">
          Manga Downloader 1989 <StatusBadge tone="warning">Demo · dữ liệu mẫu</StatusBadge>
        </p>
        <h1 className="topbar__title">{pageTitle}</h1>
      </div>
      <div className="topbar__actions">
        <Button variant="secondary" onClick={() => navigate("/discover")}>
          <Search size={18} aria-hidden="true" />
          Khám phá catalog demo
        </Button>
        <Button variant="ghost" iconOnly aria-label="Mở Settings" onClick={() => navigate("/settings")}>
          <Settings size={18} aria-hidden="true" />
        </Button>
      </div>
    </header>
  );
}
