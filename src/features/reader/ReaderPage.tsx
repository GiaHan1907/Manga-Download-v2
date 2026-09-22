import { ArrowLeft, ArrowRight, Expand, Maximize2, MoveHorizontal, ZoomIn, ZoomOut } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import type { ReaderPage as ReaderPageData } from "../../domain/reader";
import { clampPage, clampZoom, ZOOM_STEP } from "../../domain/reader";
import { services } from "../../services/provider";
import type { AppSettings } from "../../services/settings.service";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import ReaderImage from "./components/ReaderImage";

type ReaderMode = "single" | "double" | "vertical";
type FitMode = "page" | "width";

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return ["INPUT", "SELECT", "TEXTAREA", "BUTTON"].includes(target.tagName) || target.isContentEditable;
}

export default function ReaderPage() {
  const { mangaId = "", chapterId = "" } = useParams();
  const pages = useAsyncData<ReaderPageData[]>(() => services.reader.getPages(chapterId), [chapterId]);
  const sampleChapters = useAsyncData(async () => {
    const chapters = await services.catalog.getChapters(mangaId);
    return chapters.filter((chapter) => chapter.hasSamplePages);
  }, [mangaId]);
  const defaults = useAsyncData(() => services.settings.get(), []);

  if (pages.state === "loading" || defaults.state === "loading" || sampleChapters.state === "loading") {
    return <Skeleton height="70vh" label="Đang tải trang mẫu" />;
  }

  if (pages.state === "error" || pages.data.length === 0) {
    return (
      <ErrorState
        title="Chương này chưa có trang mẫu"
        description="Reader chỉ hoạt động với chapter có ảnh mẫu đóng gói trong bản demo. Không có giải mã CBZ hay tải ảnh thật."
        action={<Button to={`/manga/${mangaId}`} variant="primary">Về Manga Detail</Button>}
      />
    );
  }

  const settings = defaults.state === "success" ? defaults.data : null;

  return (
    <ReaderView
      key={`${mangaId}/${chapterId}`}
      mangaId={mangaId}
      chapterId={chapterId}
      pages={pages.data}
      sampleChapters={sampleChapters.state === "success" ? sampleChapters.data : []}
      defaults={settings}
    />
  );
}

function ReaderView({
  mangaId,
  chapterId,
  pages: list,
  sampleChapters,
  defaults,
}: {
  mangaId: string;
  chapterId: string;
  pages: ReaderPageData[];
  sampleChapters: { id: string; title: string }[];
  defaults: AppSettings | null;
}) {
  const navigate = useNavigate();
  const total = list.length;
  const [mode, setMode] = useState<ReaderMode>(defaults?.defaultReaderMode ?? "single");
  const [direction, setDirection] = useState<"ltr" | "rtl">(defaults?.defaultReadingDirection ?? "rtl");
  const [page, setPage] = useState(0);
  const [zoom, setZoom] = useState(1);
  const [fit, setFit] = useState<FitMode>("page");
  const [immersive, setImmersive] = useState(false);
  const pageRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const step = mode === "double" ? 2 : 1;

  useEffect(() => {
    let cancelled = false;
    void services.reader.getProgress(mangaId).then((progress) => {
      if (!cancelled && progress && progress.chapterId === chapterId) {
        setPage(clampPage(progress.page, total));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [mangaId, chapterId, total]);

  useEffect(() => {
    void services.reader.setProgress({ mangaId, chapterId, page });
  }, [mangaId, chapterId, page]);

  const goNext = useCallback(() => {
    setPage((current) => clampPage(current + step, total));
  }, [step, total]);

  const goPrev = useCallback(() => {
    setPage((current) => clampPage(current - step, total));
  }, [step, total]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (isTypingTarget(event.target)) {
        return;
      }
      const forward = direction === "rtl" ? "ArrowLeft" : "ArrowRight";
      const backward = direction === "rtl" ? "ArrowRight" : "ArrowLeft";
      switch (event.key) {
        case forward:
          event.preventDefault();
          goNext();
          break;
        case backward:
          event.preventDefault();
          goPrev();
          break;
        case "Home":
          setPage(0);
          break;
        case "End":
          setPage(clampPage(Number.MAX_SAFE_INTEGER, total));
          break;
        case "+":
        case "=":
          setZoom((value) => clampZoom(value + ZOOM_STEP));
          break;
        case "-":
          setZoom((value) => clampZoom(value - ZOOM_STEP));
          break;
        case "0":
          setZoom(1);
          break;
        case "f":
          setImmersive((value) => !value);
          break;
        case "Escape":
          if (immersive) {
            setImmersive(false);
          } else {
            navigate(`/manga/${mangaId}`);
          }
          break;
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [direction, goNext, goPrev, immersive, navigate, mangaId, total]);

  useEffect(() => {
    if (mode === "vertical" && list[page]) {
      pageRefs.current[list[page].id]?.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }, [mode, page, list]);

  const visiblePair = useMemo(
    () => (mode === "double" ? list.slice(page, page + 2) : list.slice(page, page + 1)),
    [mode, list, page],
  );

  const effectiveFit: FitMode = mode === "vertical" ? "width" : fit;

  return (
    <div className={`reader${immersive ? " reader--immersive" : ""}`}>
      <header className="reader__bar">
        <div className="reader__meta">
          <StatusBadge tone="warning">Sample Reader</StatusBadge>
          <span className="muted">Trang {page + 1}/{total} · ảnh nội bộ, không phải trang truyện thật</span>
        </div>
        <div className="reader__controls" aria-label="Điều khiển reader">
          <label className="reader__select" htmlFor="reader-chapter">
            Chương mẫu
            <select id="reader-chapter" className="input" value={chapterId} onChange={(event) => navigate(`/reader/${mangaId}/${event.target.value}`)}>
              {sampleChapters.map((chapter) => (
                <option key={chapter.id} value={chapter.id}>
                  {chapter.title}
                </option>
              ))}
            </select>
          </label>
          <select
            className="input"
            aria-label="Chế độ hiển thị"
            value={mode}
            onChange={(event) => {
              setMode(event.target.value as ReaderMode);
              setPage((current) => clampPage(current, total));
            }}
          >
            <option value="single">Một trang</option>
            <option value="double">Hai trang</option>
            <option value="vertical">Cuộn dọc</option>
          </select>
          <select
            className="input"
            aria-label="Hướng đọc"
            value={direction}
            onChange={(event) => setDirection(event.target.value as "ltr" | "rtl")}
          >
            <option value="rtl">RTL (truyện Nhật)</option>
            <option value="ltr">LTR</option>
          </select>
          <span className="reader__zoom" role="group" aria-label="Zoom">
            <Button variant="ghost" iconOnly aria-label="Thu nhỏ" onClick={() => setZoom((v) => clampZoom(v - ZOOM_STEP))}>
              <ZoomOut size={16} aria-hidden="true" />
            </Button>
            <span aria-live="polite">{Math.round(zoom * 100)}%</span>
            <Button variant="ghost" iconOnly aria-label="Phóng to" onClick={() => setZoom((v) => clampZoom(v + ZOOM_STEP))}>
              <ZoomIn size={16} aria-hidden="true" />
            </Button>
          </span>
          <Button variant="ghost" iconOnly aria-label="Vừa trang" aria-pressed={fit === "page"} onClick={() => { setFit("page"); setZoom(1); }}>
            <Expand size={16} aria-hidden="true" />
          </Button>
          <Button variant="ghost" iconOnly aria-label="Vừa chiều rộng" aria-pressed={fit === "width"} onClick={() => { setFit("width"); setZoom(1); }}>
            <MoveHorizontal size={16} aria-hidden="true" />
          </Button>
          <Button variant="ghost" iconOnly aria-label={immersive ? "Thoát toàn màn hình" : "Toàn màn hình (demo)"} aria-pressed={immersive} onClick={() => setImmersive((v) => !v)}>
            <Maximize2 size={16} aria-hidden="true" />
          </Button>
          <Button variant="secondary" onClick={() => navigate(`/manga/${mangaId}`)}>
            Thoát (Escape)
          </Button>
        </div>
      </header>

      <div className="reader__viewport">
        <div className="reader__nav">
          <Button variant="secondary" iconOnly disabled={page <= 0} aria-label="Trang trước" onClick={goPrev}>
            <ArrowLeft size={18} aria-hidden="true" />
          </Button>
          <Button variant="secondary" iconOnly disabled={page + step >= total} aria-label="Trang sau" onClick={goNext}>
            <ArrowRight size={18} aria-hidden="true" />
          </Button>
        </div>

        {mode === "vertical" ? (
          <div className="reader__vertical">
            {list.map((readerPage, index) => (
              <div
                key={readerPage.id}
                ref={(node) => {
                  pageRefs.current[readerPage.id] = node;
                }}
              >
                <ReaderImage page={readerPage} fit={effectiveFit} zoom={zoom} onPageIndex={index} />
              </div>
            ))}
          </div>
        ) : (
          <div className={`reader__spread reader__spread--${direction}`}>
            {visiblePair.map((readerPage, index) => (
              <ReaderImage key={readerPage.id} page={readerPage} fit={effectiveFit} zoom={zoom} onPageIndex={page + index} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
