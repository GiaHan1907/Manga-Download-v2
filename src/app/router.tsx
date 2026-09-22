import { HashRouter, Route, Routes } from "react-router-dom";
import AppShell from "../components/layout/AppShell";
import ErrorState from "../components/ui/ErrorState";
import Button from "../components/ui/Button";
import DiscoverPage from "../features/discover/DiscoverPage";
import DownloadsPage from "../features/downloads/DownloadsPage";
import HomePage from "../features/home/HomePage";
import FavoritesPage from "../features/library/FavoritesPage";
import LibraryPage from "../features/library/LibraryPage";
import MangaDetailPage from "../features/manga-detail/MangaDetailPage";
import ReaderPage from "../features/reader/ReaderPage";
import SettingsPage from "../features/settings/SettingsPage";
import SourcesPage from "../features/settings/SourcesPage";

export function AppRoutes() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<HomePage />} />
        <Route path="/discover" element={<DiscoverPage />} />
        <Route path="/library" element={<LibraryPage />} />
        <Route path="/favorites" element={<FavoritesPage />} />
        <Route path="/downloads" element={<DownloadsPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="/settings/sources" element={<SourcesPage />} />
        <Route path="/manga/:mangaId" element={<MangaDetailPage />} />
        <Route path="/reader/:mangaId/:chapterId" element={<ReaderPage />} />
        <Route path="*" element={<NotFoundPage />} />
      </Route>
    </Routes>
  );
}

export default function AppRouter() {
  return (
    <HashRouter>
      <AppRoutes />
    </HashRouter>
  );
}

function NotFoundPage() {
  return (
    <ErrorState
      title="Không tìm thấy trang"
      description="Route này không tồn tại trong prototype Phase 03. Hãy quay về Home hoặc chọn một mục trong sidebar."
      action={<Button to="/" variant="primary">Về Home</Button>}
    />
  );
}
