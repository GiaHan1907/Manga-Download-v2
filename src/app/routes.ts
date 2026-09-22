export type AppRouteId =
  | "home"
  | "discover"
  | "library"
  | "downloads"
  | "favorites"
  | "settings"
  | "manga-detail"
  | "reader";

export type AppRoute = {
  id: AppRouteId;
  path: string;
  label: string;
  description: string;
  sidebar: boolean;
  phase: "Phase 02" | "Phase 03";
};

export const appRoutes = [
  {
    id: "home",
    path: "/",
    label: "Home",
    description: "Tổng quan nền tảng và trạng thái hệ thống Phase 01.",
    sidebar: true,
    phase: "Phase 02",
  },
  {
    id: "discover",
    path: "/discover",
    label: "Discover",
    description: "Khung điều hướng khám phá truyện, chưa kết nối nguồn dữ liệu.",
    sidebar: true,
    phase: "Phase 03",
  },
  {
    id: "library",
    path: "/library",
    label: "Library",
    description: "Khung thư viện local-first cho các phase sau.",
    sidebar: true,
    phase: "Phase 03",
  },
  {
    id: "downloads",
    path: "/downloads",
    label: "Downloads",
    description: "Khung hàng đợi tải, chưa có downloader hoặc tiến độ thật.",
    sidebar: true,
    phase: "Phase 03",
  },
  {
    id: "favorites",
    path: "/favorites",
    label: "Favorites",
    description: "Khung danh sách yêu thích cho dữ liệu người dùng ở phase sau.",
    sidebar: true,
    phase: "Phase 03",
  },
  {
    id: "settings",
    path: "/settings",
    label: "Settings",
    description: "Khung cài đặt ứng dụng, chưa ghi setting thật vào database.",
    sidebar: true,
    phase: "Phase 03",
  },
  {
    id: "manga-detail",
    path: "/manga/demo",
    label: "Manga Detail",
    description: "Placeholder màn hình chi tiết truyện, không tải dữ liệu thật.",
    sidebar: false,
    phase: "Phase 03",
  },
  {
    id: "reader",
    path: "/reader/demo",
    label: "Reader",
    description: "Placeholder trình đọc offline, chưa có reader engine.",
    sidebar: false,
    phase: "Phase 03",
  },
] satisfies AppRoute[];

export const sidebarRoutes = appRoutes.filter((route) => route.sidebar);

export function getRouteByPath(pathname: string) {
  return appRoutes.find((route) => route.path === pathname) ?? appRoutes[0];
}
