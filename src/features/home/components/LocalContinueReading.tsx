import { History } from "lucide-react";
import Button from "../../../components/ui/Button";
import { localServices } from "../../../services/provider";
import { useAsyncData } from "../../../shared/hooks/useAsyncData";
import { useCoverUrl } from "../../../shared/hooks/useCoverUrl";
import { useDomainVersion } from "../../../shared/hooks/useInvalidation";
import type { LibraryMangaDto } from "../../../types/library";

/// Real (SQLite-backed) Continue Reading — UI plan C4: the most recent read
/// is a hero card, the rest stay a compact strip. Refetches whenever another
/// screen bumps the `home` domain. Data logic unchanged (lastRead + already
/// downloaded + has progress).
export default function LocalContinueReading() {
  const homeVersion = useDomainVersion("home");
  const page = useAsyncData(
    () => localServices.libraryV2.list({ sort: "lastRead", filter: "downloaded", limit: 3, offset: 0 }),
    [homeVersion],
  );
  if (page.state !== "success") {
    return null;
  }
  const items: LibraryMangaDto[] = page.data.items.filter((manga) => manga.progress !== null);
  if (items.length === 0) {
    return null;
  }
  const [hero, ...rest] = items;
  return (
    <section className="panel continue-home" aria-label="Đang đọc tiếp (local)">
      <h2 className="panel__title">
        <History size={16} aria-hidden="true" /> Đang đọc tiếp
      </h2>
      <ContinueHero manga={hero} />
      {rest.length > 0 && (
        <ul className="continue-local">
          {rest.map((manga) => (
            <li key={manga.id} className="continue-local__row">
              <span className="continue-local__title">{manga.title}</span>
              <span className="muted">{describeProgress(manga)}</span>
              <Button to={`/read/${manga.progress?.chapterId}`} variant="secondary">
                Đọc tiếp
              </Button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function describeProgress(manga: LibraryMangaDto): string {
  const total = manga.progress?.pageTotal ?? 0;
  const pageAt = (manga.progress?.pageIndex ?? 0) + 1;
  const pct = total > 0 ? Math.min(100, Math.round((pageAt / total) * 100)) : 0;
  return `${total > 0 ? `trang ${pageAt}/${total}` : ""}${total > 0 ? " · " : ""}${pct}%`;
}

function ContinueHero({ manga }: { manga: LibraryMangaDto }) {
  const total = manga.progress?.pageTotal ?? 0;
  const pct = total > 0 ? Math.min(100, Math.round((((manga.progress?.pageIndex ?? 0) + 1) / total) * 100)) : 0;
  const coverUrl = useCoverUrl(manga.sourceId, manga.sourceMangaId, manga.coverKey);
  return (
    <div className="continue-hero" data-testid="continue-hero">
      <div className="continue-hero__cover" aria-hidden="true">
        {coverUrl ? <img src={coverUrl} alt="" /> : manga.title.charAt(0).toUpperCase()}
      </div>
      <div className="continue-hero__body">
        <h3 className="continue-hero__title">{manga.title}</h3>
        <p className="muted">
          {manga.author ? `${manga.author} · ` : ""}
          {describeProgress(manga)}
        </p>
        <div className="continue-hero__bar" role="presentation">
          <span style={{ width: `${pct}%` }} />
        </div>
        <div className="continue-hero__actions">
          <Button to={`/read/${manga.progress?.chapterId}`} variant="primary">
            Đọc tiếp
          </Button>
          <Button to={`/library/${manga.id}`} variant="ghost">
            Mở truyện
          </Button>
        </div>
      </div>
    </div>
  );
}
