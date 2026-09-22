import { BookOpen, Compass, Download, Heart, Home, Library, Settings, Sparkles } from "lucide-react";
import type { AppRoute } from "../../app/routes";
import Button from "../../components/ui/Button";
import ChapterRow from "../../components/ui/ChapterRow";
import DownloadCard from "../../components/ui/DownloadCard";
import EmptyState from "../../components/ui/EmptyState";
import Input from "../../components/ui/Input";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import Tabs from "../../components/ui/Tabs";
import SystemStatus from "../system/SystemStatus";

const iconByRoute = {
  home: Home,
  discover: Compass,
  library: Library,
  downloads: Download,
  favorites: Heart,
  settings: Settings,
  "manga-detail": BookOpen,
  reader: BookOpen,
} satisfies Record<AppRoute["id"], typeof Home>;

export type PlaceholderPageProps = {
  route: AppRoute;
};

export default function PlaceholderPage({ route }: PlaceholderPageProps) {
  const Icon = iconByRoute[route.id];

  if (route.id === "home") {
    return <HomePage route={route} />;
  }

  return (
    <div className="page-stack">
      <EmptyState
        title={`${route.label} đang chuẩn bị`}
        description={`${route.description} Coming in ${route.phase === "Phase 02" ? "Phase 02" : "Phase 03"}.`}
        action={<StatusBadge tone="warning">Không có dữ liệu thật</StatusBadge>}
      />
      <section className="panel">
        <Icon size={24} aria-hidden="true" />
        <h2 className="panel__title" style={{ marginTop: "var(--space-3)" }}>
          Ranh giới chức năng
        </h2>
        <p className="panel__description">
          Trang này chỉ kiểm tra layout, navigation và trạng thái UI. Search, import URL, downloader, library data và reader engine sẽ được triển khai ở phase sau.
        </p>
      </section>
      {route.id === "discover" && (
        <section className="panel">
          <Input label="Search preview" placeholder="Tìm manga trong Phase 03" disabled hint="Ô này chỉ là affordance thị giác, chưa gửi truy vấn." />
        </section>
      )}
      {route.id === "downloads" && (
        <section className="component-grid" aria-label="Mẫu thẻ tải xuống">
          <DownloadCard title="Mẫu hàng đợi tải" description="Fixture UI, không có downloader thật." progress={42} status="Fixture" />
        </section>
      )}
      {route.id === "manga-detail" && (
        <section className="page-stack">
          <MangaCard title="Một tựa manga rất dài dùng để kiểm tra wrapping tiếng Việt và 日本語" subtitle="Fixture card cho layout 2:3" />
          <ChapterRow title="Chapter 001 — Placeholder" meta="Không có nội dung reader thật trong Phase 02" />
        </section>
      )}
    </div>
  );
}

function HomePage({ route }: PlaceholderPageProps) {
  return (
    <div className="page-stack">
      <section className="panel">
        <Sparkles size={24} aria-hidden="true" />
        <h1 className="panel__title" style={{ marginTop: "var(--space-3)" }}>
          {route.label}
        </h1>
        <p className="panel__description">{route.description}</p>
      </section>
      <SystemStatus />
      <section className="component-grid" aria-label="Component preview bằng fixture">
        <MangaCard title="Thiết kế card manga — 日本語タイトル kiểm tra dòng dài" subtitle="Fixture UI, chưa kết nối nguồn manga" />
        <DownloadCard title="Trạng thái tải mẫu" description="Thanh tiến độ chỉ phục vụ preview component." progress={64} status="Fixture" />
        <div className="panel">
          <h2 className="panel__title">Loading primitives</h2>
          <div className="page-stack" style={{ gap: "var(--space-3)" }}>
            <Skeleton height="1rem" />
            <Skeleton height="1rem" width="75%" />
            <Skeleton height="5rem" />
          </div>
        </div>
      </section>
      <Tabs
        items={[
          {
            id: "scope",
            label: "Scope",
            content: <p className="muted">Phase 02 chỉ dựng shell, routing và UI system.</p>,
          },
          {
            id: "next",
            label: "Phase 03",
            content: <p className="muted">Search, import URL, downloader và reader sẽ cần backend/data model riêng.</p>,
          },
        ]}
      />
      <Button to="/discover" variant="primary">
        Đi tới Discover placeholder
      </Button>
    </div>
  );
}
