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
  { id: "home", path: "/", label: "Home", description: "Bảng điều khiển demo từ mock state + health IPC thật.", sidebar: true },
  { id: "discover", path: "/discover", label: "Discover", description: "Tìm kiếm + import URL thật qua Rust source engine; demo sandbox tách riêng.", sidebar: true },
  { id: "library", path: "/library", label: "Library", description: "Thư viện mẫu trong mock state của phiên.", sidebar: true },
  { id: "downloads", path: "/downloads", label: "Downloads", description: "Hàng đợi demo với transition do người dùng điều khiển.", sidebar: true },
  { id: "favorites", path: "/favorites", label: "Favorites", description: "Danh sách yêu thích dùng chung mock state.", sidebar: true },
  { id: "settings", path: "/settings", label: "Settings", description: "Cài đặt demo, validate và lưu trong mock state.", sidebar: true },
  { id: "sources", path: "/settings/sources", label: "Sources", description: "Bật/tắt nguồn demo, ảnh hưởng bộ lọc Discover.", sidebar: false },
  { id: "manga-detail", path: "/manga/:mangaId", label: "Manga Detail", description: "Chi tiết manga và chọn chương demo.", sidebar: false },
  { id: "remote-detail", path: "/remote/:sourceId/:sourceMangaId", label: "Remote Preview", description: "Xem trước metadata/chương từ nguồn thật; lưu metadata khi người dùng bấm.", sidebar: false },
  { id: "reader", path: "/reader/:mangaId/:chapterId", label: "Reader", description: "Sample reader UI với ảnh trung tính nội bộ.", sidebar: false },
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
