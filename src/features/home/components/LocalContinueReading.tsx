import { History } from "lucide-react";
import Button from "../../../components/ui/Button";
import { localServices } from "../../../services/provider";
import { useAsyncData } from "../../../shared/hooks/useAsyncData";
import type { LibraryMangaDto } from "../../../types/library";

/// Real (SQLite-backed) Continue Reading strip — last-read manga with saved
/// progress. Separate from the labeled demo panel below it on Home.
export default function LocalContinueReading() {
  const page = useAsyncData(
    () => localServices.libraryV2.list({ sort: "lastRead", filter: "downloaded", limit: 3, offset: 0 }),
    [],
  );
  if (page.state !== "success") {
    return null;
  }
  const items: LibraryMangaDto[] = page.data.items.filter((manga) => manga.progress !== null);
  if (items.length === 0) {
    return null;
  }
  return (
    <section className="panel" aria-label="Đọc tiếp (local)">
      <h2 className="panel__title">
        <History size={16} aria-hidden="true" /> Đọc tiếp — dữ liệu thật
      </h2>
      <ul className="continue-local">
        {items.map((manga) => {
          const total = manga.progress?.pageTotal ?? 0;
          const pct = total > 0 ? Math.min(100, Math.round((((manga.progress?.pageIndex ?? 0) + 1) / total) * 100)) : 0;
          return (
            <li key={manga.id} className="continue-local__row">
              <span className="continue-local__title">{manga.title}</span>
              <span className="muted">
                {manga.progress?.pageIndex !== undefined ? `trang ${(manga.progress?.pageIndex ?? 0) + 1}/${total || "?"}` : ""}
                {" · "}
                {pct}%
              </span>
              <Button to={`/library/${manga.id}`} variant="secondary">
                Mở truyện
              </Button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
