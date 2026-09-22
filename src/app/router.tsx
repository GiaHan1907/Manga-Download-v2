import { HashRouter, Route, Routes } from "react-router-dom";
import AppShell from "../components/layout/AppShell";
import ErrorState from "../components/ui/ErrorState";
import Button from "../components/ui/Button";
import PlaceholderPage from "../features/placeholders/PlaceholderPage";
import { appRoutes } from "./routes";

export function AppRoutes() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        {appRoutes.map((route) => (
          <Route key={route.id} path={route.path} element={<PlaceholderPage route={route} />} />
        ))}
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
      description="Route này chưa tồn tại trong UI shell Phase 02. Hãy quay về Home hoặc chọn một mục trong sidebar."
      action={<Button to="/" variant="primary">Về Home</Button>}
    />
  );
}
