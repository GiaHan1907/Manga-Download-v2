import { ClipboardList } from "lucide-react";
import { useEffect, useState } from "react";
import Button from "../../../components/ui/Button";
import ErrorState from "../../../components/ui/ErrorState";
import Input from "../../../components/ui/Input";
import MangaCard from "../../../components/ui/MangaCard";
import type { MangaSummary } from "../../../domain/manga";
import { DEMO_SOURCE_PATTERN, type CatalogService } from "../../../services/catalog.service";

type ImportFeedback =
  | { kind: "idle" }
  | { kind: "ok"; manga: MangaSummary }
  | { kind: "error"; message: string };

function classifyUrlError(raw: string): string | null {
  const url = raw.trim();
  if (!url) {
    return "Hãy nhập một URL để kiểm thử import demo.";
  }
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      return `URL không hợp lệ: scheme "${parsed.protocol}" không được hỗ trợ.`;
    }
  } catch {
    return "URL không hợp lệ — không thể phân tích cú pháp.";
  }
  if (!DEMO_SOURCE_PATTERN.test(url)) {
    return "Nguồn không được hỗ trợ. Bản demo chỉ nhận URL mẫu thuộc demo.manga1989.example.";
  }
  return null;
}

export type UrlImportPanelProps = {
  catalog: CatalogService;
};

export default function UrlImportPanel({ catalog }: UrlImportPanelProps) {
  const [url, setUrl] = useState("");
  const [exampleUrl, setExampleUrl] = useState("");
  const [feedback, setFeedback] = useState<ImportFeedback>({ kind: "idle" });

  useEffect(() => {
    let alive = true;
    catalog.getDemoExampleUrl().then((value) => {
      if (alive) {
        setExampleUrl(value);
      }
    });
    return () => {
      alive = false;
    };
  }, [catalog]);

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    const error = classifyUrlError(url);
    if (error) {
      setFeedback({ kind: "error", message: error });
      return;
    }
    const manga = await catalog.resolveDemoUrl(url);
    if (!manga) {
      setFeedback({ kind: "error", message: "Không tìm thấy manga mẫu tương ứng với URL này." });
      return;
    }
    setFeedback({ kind: "ok", manga });
  }

  return (
    <div className="page-stack">
      <p className="muted">
        <ClipboardList size={14} aria-hidden="true" /> Demo import — supported sample URL only. Không có truy vấn mạng nào được thực hiện.
      </p>
      <form className="url-import-form" onSubmit={(event) => void handleSubmit(event)}>
        <Input
          label="URL mẫu demo"
          placeholder="https://demo.manga1989.example/manga/manga-001"
          value={url}
          onChange={(event) => setUrl(event.target.value)}
          hint="Chỉ khớp fixture nội bộ; URL khác sẽ bị từ chối kèm lý do."
        />
        <div className="url-import-form__actions">
          <Button type="submit" variant="primary">
            Kiểm tra URL
          </Button>
          <Button type="button" variant="ghost" onClick={() => setUrl(exampleUrl)}>
            Dùng URL mẫu
          </Button>
        </div>
      </form>

      {feedback.kind === "error" && (
        <ErrorState title="Import không thành công" description={feedback.message} />
      )}
      {feedback.kind === "ok" && (
        <section className="panel" aria-label="Kết quả import demo">
          <h3 className="panel__title">Khớp fixture demo</h3>
          <div className="url-import-result">
            <MangaCard
              title={feedback.manga.title}
              subtitle={`${feedback.manga.author ?? "Không rõ tác giả"} · ${feedback.manga.sourceId}`}
              status={feedback.manga.status}
              coverUrl={feedback.manga.coverAsset}
            />
            <Button to={`/manga/${feedback.manga.id}`} variant="primary">
              Mở Manga Detail
            </Button>
          </div>
        </section>
      )}
    </div>
  );
}
