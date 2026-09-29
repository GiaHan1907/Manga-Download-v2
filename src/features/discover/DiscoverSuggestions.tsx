import { Plus } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import Button from "../../components/ui/Button";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import { localServices } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";
import type { HomeFeedItemDto } from "../../types/source";

/// The Home source's own spotlight rail, reused as Discover's starting point.
/// Labelled "Nổi bật trên TruyenQQ" on purpose: there is no preference model
/// behind it, so it must not be presented as personal recommendation.
const SOURCE_ID = "truyenqq";

function SuggestionCard({ item }: { item: HomeFeedItemDto }) {
  const url = useCoverUrl(item.sourceId, item.sourceMangaId, item.coverKey);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function save() {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      const result = await localServices.source.save({
        sourceId: item.sourceId,
        sourceMangaId: item.sourceMangaId,
      });
      setNote(`Đã lưu ${result.savedChapterCount} chương`);
      bumpDomain("library");
    } catch (caught: unknown) {
      setError(normalizeIpcError(caught).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <li className="discover-suggestion">
      <Link
        to={`/remote/${encodeURIComponent(item.sourceId)}/${encodeURIComponent(item.sourceMangaId)}`}
        className="card-link"
        aria-label={`Mở xem trước ${item.title}`}
      >
        <MangaCard
          title={item.title}
          subtitle={item.sourceDisplayName}
          status={item.latestChapter ?? "Chưa có chương nào"}
          coverUrl={url ?? undefined}
        />
      </Link>
      <Button
        variant="primary"
        iconOnly
        className="discover-suggestion__add"
        aria-label={`Thêm ${item.title} vào thư viện`}
        onClick={() => void save()}
        disabled={busy}
      >
        <Plus size={16} aria-hidden="true" />
      </Button>
      {note && (
        <span className="discover-suggestion__note" role="status">
          {note}
        </span>
      )}
      {error && (
        <span className="queue-error discover-suggestion__error" role="alert">
          {error}
        </span>
      )}
    </li>
  );
}

export default function DiscoverSuggestions() {
  const homeVersion = useDomainVersion("home");
  const feed = useAsyncData(() => localServices.source.homeFeed(SOURCE_ID), [homeVersion]);

  if (feed.state === "loading") {
    return (
      <div className="card-grid" aria-busy="true">
        <Skeleton height="10rem" />
        <Skeleton height="10rem" />
        <Skeleton height="10rem" />
      </div>
    );
  }
  if (feed.state === "error") {
    return (
      <p className="queue-error" role="alert">
        {normalizeIpcError(feed.error).message}
      </p>
    );
  }
  if (feed.data.featured.length === 0) {
    return <p className="muted">Nguồn chưa có truyện nổi bật để gợi ý.</p>;
  }
  return (
    <ul className="card-grid discover-suggestions" aria-label="Truyện nổi bật trên TruyenQQ">
      {feed.data.featured.slice(0, 8).map((item) => (
        <SuggestionCard key={item.sourceMangaId} item={item} />
      ))}
    </ul>
  );
}
