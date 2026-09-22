import { BookMarked, Compass, Download, Globe, Heart, Home, Library, Menu, Server, Settings } from "lucide-react";
import { useRef } from "react";
import { NavLink } from "react-router-dom";
import { sidebarRoutes, type AppRoute } from "../../app/routes";
import Button from "../ui/Button";
import Tooltip from "../ui/Tooltip";

const icons = {
  home: Home,
  discover: Compass,
  library: Library,
  downloads: Download,
  favorites: Heart,
  settings: Settings,
  sources: Server,
  "manga-detail": BookMarked,
  "remote-detail": Globe,
  reader: BookMarked,
} satisfies Record<AppRoute["id"], typeof Home>;

export type SidebarProps = {
  collapsed: boolean;
  onToggleCollapsed: () => void;
};

export default function Sidebar({ collapsed, onToggleCollapsed }: SidebarProps) {
  const navRef = useRef<HTMLElement>(null);

  function handleNavKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
      return;
    }

    const links = Array.from(navRef.current?.querySelectorAll<HTMLAnchorElement>("a[href]") ?? []);
    const currentIndex = links.findIndex((link) => link === document.activeElement);
    if (currentIndex < 0) {
      return;
    }

    event.preventDefault();
    const direction = event.key === "ArrowDown" ? 1 : -1;
    links[(currentIndex + direction + links.length) % links.length].focus();
  }

  return (
    <aside className="sidebar" aria-label="Điều hướng chính">
      <div className="sidebar__header">
        <span className="sidebar__eyebrow">Navigation</span>
        <Button variant="ghost" iconOnly aria-label={collapsed ? "Mở rộng sidebar" : "Thu gọn sidebar"} onClick={onToggleCollapsed}>
          <Menu size={18} aria-hidden="true" />
        </Button>
      </div>
      <nav ref={navRef} className="sidebar__nav" onKeyDown={handleNavKeyDown}>
        {sidebarRoutes.map((route) => {
          const Icon = icons[route.id];
          const link = (
            <NavLink
              key={route.id}
              to={route.path}
              end={route.path === "/"}
              className={({ isActive }) => `sidebar__link${isActive ? " sidebar__link--active" : ""}`}
              aria-label={collapsed ? route.label : undefined}
            >
              <Icon size={20} aria-hidden="true" />
              <span className="sidebar__label">{route.label}</span>
            </NavLink>
          );

          return collapsed ? (
            <Tooltip key={route.id} content={route.label}>
              {link}
            </Tooltip>
          ) : (
            link
          );
        })}
      </nav>
    </aside>
  );
}
