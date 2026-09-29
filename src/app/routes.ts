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
  { id: "home", path: "/", label: "Trang chủ", description: "Tổng quan: đọc tiếp, hoạt động, thống kê từ SQLite.", sidebar: true },
  { id: "discover", path: "/discover", label: "Khám phá", description: "Tìm kiếm + import URL qua Rust source engine; demo sandbox chỉ ở bản build dev.", sidebar: true },
  { id: "library", path: "/library", label: "Thư viện", description: "Truyện đã lưu, trạng thái offline, reconcile.", sidebar: true },
  { id: "downloads", path: "/downloads", label: "Tải xuống", description: "Hàng đợi qua download engine Rust với pause/cancel/retry.", sidebar: true },
  { id: "favorites", path: "/favorites", label: "Yêu thích", description: "Danh sách yêu thích lưu trong SQLite.", sidebar: true },
  { id: "settings", path: "/settings", label: "Cài đặt", description: "Cài đặt, validate Rust-side và lưu blob app_settings.", sidebar: true },
  { id: "sources", path: "/settings/sources", label: "Nguồn", description: "Registry nguồn (bật/tắt, Madara tùy biến); sandbox demo dev-only.", sidebar: false },
  { id: "manga-detail", path: "/manga/:mangaId", label: "Chi tiết truyện", description: "Chi tiết manga đã lưu + danh sách chương offline.", sidebar: false },
  { id: "remote-detail", path: "/remote/:sourceId/:sourceMangaId", label: "Xem trước nguồn", description: "Xem trước metadata/chương từ nguồn; lưu metadata khi người dùng bấm.", sidebar: false },
  { id: "reader", path: "/reader/:mangaId/:chapterId", label: "Trang đọc", description: "Trang dev-reader (mock); reader chính là /read/:chapterId.", sidebar: false },
] satisfies AppRoute[];

export const sidebarRoutes = appRoutes.filter((route) => route.sidebar);

export function resolvePageTitle(pathname: string): string {
  if (pathname === "/") return "Trang chủ";
  if (pathname.startsWith("/manga/")) return "Chi tiết truyện";
  if (pathname.startsWith("/remote/")) return "Xem trước nguồn";
  if (pathname.startsWith("/reader/")) return "Trang đọc";
  if (pathname === "/settings/sources") return "Nguồn";
  const match = appRoutes.find((route) => route.path === pathname);
  return match?.label ?? "Manga Downloader 1989";
}
