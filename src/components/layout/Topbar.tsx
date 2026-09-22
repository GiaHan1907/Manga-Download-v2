import { Search, Settings } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";
import { getRouteByPath } from "../../app/routes";
import Button from "../ui/Button";

export default function Topbar() {
  const location = useLocation();
  const navigate = useNavigate();
  const currentRoute = getRouteByPath(location.pathname);

  return (
    <header className="topbar">
      <div>
        <p className="muted">Manga Downloader 1989</p>
        <h1 className="topbar__title">{currentRoute.label}</h1>
      </div>
      <div className="topbar__actions">
        <Button variant="secondary" onClick={() => navigate("/discover")}>
          <Search size={18} aria-hidden="true" />
          Discover placeholder
        </Button>
        <Button variant="ghost" iconOnly aria-label="Mở Settings placeholder" onClick={() => navigate("/settings")}>
          <Settings size={18} aria-hidden="true" />
        </Button>
      </div>
    </header>
  );
}
