export type AppRouteId =
  | "home"
  | "discover"
  | "library"
  | "downloads"
  | "favorites"
  | "settings"
  | "sources"
  | "manga-detail"
  | "remote-detail"
  | "reader";

export type AppRoute = {
  id: AppRouteId;
  path: string;
  label: string;
  description: string;
  sidebar: boolean;
};

export const appRoutes = [
  { id: "home", path: "/", label: "Home", description: "Tổng quan thật: continue reading, activity, stats từ SQLite.", sidebar: true },
  { id: "discover", path: "/discover", label: "Discover", description: "Tìm kiếm + import URL thật qua Rust source engine; demo sandbox chỉ ở bản build dev.", sidebar: true },
  { id: "library", path: "/library", label: "Library", description: "Thư viện thật: truyện đã lưu, trạng thái offline, reconcile.", sidebar: true },
  { id: "downloads", path: "/downloads", label: "Downloads", description: "Hàng đợi thật qua download engine Rust với pause/cancel/retry.", sidebar: true },
  { id: "favorites", path: "/favorites", label: "Favorites", description: "Danh sách yêu thích lưu trong SQLite.", sidebar: true },
  { id: "settings", path: "/settings", label: "Settings", description: "Cài đặt thật, validate Rust-side và lưu blob app_settings.", sidebar: true },
  { id: "sources", path: "/settings/sources", label: "Sources", description: "Registry nguồn thật (bật/tắt, Madara tùy biến); sandbox demo dev-only.", sidebar: false },
  { id: "manga-detail", path: "/manga/:mangaId", label: "Manga Detail", description: "Chi tiết manga đã lưu + danh sách chương offline.", sidebar: false },
  { id: "remote-detail", path: "/remote/:sourceId/:sourceMangaId", label: "Remote Preview", description: "Xem trước metadata/chương từ nguồn thật; lưu metadata khi người dùng bấm.", sidebar: false },
  { id: "reader", path: "/reader/:mangaId/:chapterId", label: "Reader", description: "Trang dev-reader (mock); reader thật là /read/:chapterId.", sidebar: false },
] satisfies AppRoute[];

export const sidebarRoutes = appRoutes.filter((route) => route.sidebar);

export function resolvePageTitle(pathname: string): string {
  if (pathname === "/") return "Home";
  if (pathname.startsWith("/manga/")) return "Manga Detail";
  if (pathname.startsWith("/remote/")) return "Remote Preview";
  if (pathname.startsWith("/reader/")) return "Reader";
  if (pathname === "/settings/sources") return "Sources";
  const match = appRoutes.find((route) => route.path === pathname);
  return match?.label ?? "Manga Downloader 1989";
}
