import {
  ArrowLeft,
  Maximize,
  Minimize,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import type { ReaderManifestDto } from "../../types/library";
import { localServices } from "../../services/provider";
import { settingsApi } from "../../services/settings.api";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { clampPage, nextPage, prevPage } from "./readerLogic";
import { usePageImages } from "./usePageImages";
import { useReaderProgress } from "./useReaderProgress";
import ReaderModes from "./ReaderModes";

type ReaderMode = "single" | "double" | "vertical";
type Direction = "ltr" | "rtl";

const MODES: ReaderMode[] = ["single", "double", "vertical"];
const MODE_LABELS: Record<ReaderMode, string> = {
  single: "Một trang",
  double: "Hai trang",
  vertical: "Cuộn dọc",
};

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName) || target.isContentEditable;
}

function guessMime(bytes: Uint8Array): string {
  if (bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47) return "image/png";
  if (bytes[0] === 0xff && bytes[1] === 0xd8) return "image/jpeg";
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46) return "image/gif";
  return "image/webp";
}

export default function ReadPage() {
  const { chapterId = "" } = useParams();
  const navigate = useNavigate();
  const manifest = useAsyncData<ReaderManifestDto>(
    () => localServices.reader.openChapter(chapterId),
    [chapterId],
  );

  const [page, setPage] = useState(0);
  const [mode, setMode] = useState<ReaderMode>("single");
  const [direction, setDirection] = useState<Direction>("rtl");
  const [zoom, setZoom] = useState(1);
  const [chromeVisible, setChromeVisible] = useState(true);
  const [fullscreen, setFullscreen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const hideTimer = useRef<number | null>(null);
  const initialized = useRef(false);

  // Defaults from real Settings (Phase 04) once; user can override per session.
  useEffect(() => {
    let cancelled = false;
    void settingsApi
      .get()
      .then((value) => {
        if (cancelled) {
          return;
        }
        setMode(
          value.defaultReaderMode === "vertical"
            ? "vertical"
            : value.defaultReaderMode === "double"
              ? "double"
              : "single",
        );
        setDirection(value.defaultReadingDirection === "ltr" ? "ltr" : "rtl");
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  const fetchPage = useCallback(
    async (index: number) => {
      const bytes = await localServices.reader.getPageBytes(chapterId, index);
      return { bytes, mime: guessMime(bytes) };
    },
    [chapterId],
  );

  const pageCount = manifest.state === "success" ? manifest.data.pageCount : 0;
  const mangaId = manifest.state === "success" ? manifest.data.mangaId : "";
  const startPage = manifest.state === "success" ? manifest.data.startPage : 0;
  const { urls, errors, retry } = usePageImages(chapterId, page, mode, pageCount, fetchPage);

  // Once the manifest resolves, jump to the stored resume position.
  useEffect(() => {
    if (manifest.state === "success" && !initialized.current) {
      initialized.current = true;
      setPage(clampPage(startPage, pageCount || 1));
    }
  }, [manifest.state, startPage, pageCount]);

  const goTo = useCallback(
    (candidate: number) => setPage(clampPage(candidate, pageCount || 1)),
    [pageCount],
  );
  const stepForward = useCallback(() => {
    setPage((current) => (pageCount ? nextPage(current, pageCount) : current));
  }, [pageCount]);
  const stepBack = useCallback(() => {
    setPage((current) => (pageCount ? prevPage(current) : current));
  }, [pageCount]);

  useReaderProgress(manifest, page);

  // Keyboard: RTL swaps left/right visual navigation (tested explicitly).
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (isTyping(event.target)) {
        return;
      }
      const forward = direction === "rtl" ? event.key === "ArrowLeft" : event.key === "ArrowRight";
      const backward = direction === "rtl" ? event.key === "ArrowRight" : event.key === "ArrowLeft";
      if (forward || event.key === "PageDown" || event.key === " ") {
        event.preventDefault();
        stepForward();
      } else if (backward || event.key === "PageUp") {
        event.preventDefault();
        stepBack();
      } else if (event.key === "Home") {
        goTo(0);
      } else if (event.key === "End") {
        goTo(pageCount - 1);
      } else if (event.key === "+" || event.key === "=") {
        setZoom((value) => Math.min(3, +(value + 0.25).toFixed(2)));
      } else if (event.key === "-") {
        setZoom((value) => Math.max(0.5, +(value - 0.25).toFixed(2)));
      } else if (event.key === "0") {
        setZoom(1);
      } else if (event.key === "t" || event.key === "T") {
        setChromeVisible((value) => !value);
      } else if (event.key === "f" || event.key === "F" || event.key === "F11") {
        event.preventDefault();
        void toggleFullscreen();
      } else if (event.key === "Escape") {
        if (document.fullscreenElement) {
          void document.exitFullscreen();
        } else {
          navigate(`/library/${mangaId}`);
        }
      } else if (event.key === "m" || event.key === "M") {
        setMode((value) => (value === "single" ? "double" : value === "double" ? "vertical" : "single"));
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [direction, pageCount, stepForward, stepBack, goTo, navigate, mangaId]);

  function toggleChromeWithActivity() {
    setChromeVisible(true);
    if (hideTimer.current) {
      window.clearTimeout(hideTimer.current);
    }
    hideTimer.current = window.setTimeout(() => setChromeVisible(false), 3000);
  }

  async function toggleFullscreen() {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else if (containerRef.current) {
        await containerRef.current.requestFullscreen();
      }
    } catch {
      // WebView/OS may refuse real fullscreen — CSS pseudo-fullscreen below.
      setFullscreen((value) => !value);
    }
  }

  useEffect(() => {
    const sync = () => setFullscreen(Boolean(document.fullscreenElement));
    document.addEventListener("fullscreenchange", sync);
    return () => document.removeEventListener("fullscreenchange", sync);
  }, []);

  if (manifest.state === "loading") {
    return <Skeleton height="80vh" label="Đang mở chương" />;
  }
  if (manifest.state === "error" || !manifest.data) {
    return (
      <ErrorState
        title="Không mở được chương"
        description={
          manifest.state === "error"
            ? manifest.error instanceof Error
              ? manifest.error.message
              : "Chương có thể thiếu file hoặc archive lỗi."
            : "Không tìm thấy chương."
        }
        action={
          <button type="button" className="btn btn--secondary" onClick={() => navigate(-1)}>
            Quay lại
          </button>
        }
      />
    );
  }
  const data = manifest.data;

  return (
    <div
      ref={containerRef}
      className={`reader-shell${fullscreen || document.fullscreenElement ? " reader-shell--full" : ""}`}
      onMouseMove={toggleChromeWithActivity}
      onClick={toggleChromeWithActivity}
      data-testid="reader-shell"
    >
      <header className={`reader-toolbar${chromeVisible ? "" : " reader-toolbar--hidden"}`}>
        <button
          type="button"
          className="reader-tool"
          onClick={() => navigate(`/library/${data.mangaId}`)}
          aria-label="Quay lại truyện"
        >
          <ArrowLeft size={18} aria-hidden="true" />
        </button>
        <div className="reader-toolbar__titles">
          <span className="reader-toolbar__manga">{data.mangaTitle}</span>
          <span className="reader-toolbar__chapter">{data.chapterTitle}</span>
        </div>
        <div className="reader-toolbar__controls">
          <span data-testid="reader-page-indicator">
            {page + 1}/{data.pageCount}
          </span>
          {MODES.map((value) => (
            <button
              key={value}
              type="button"
              className={`reader-tool${mode === value ? " reader-tool--active" : ""}`}
              onClick={() => setMode(value)}
              aria-label={MODE_LABELS[value]}
            >
              {MODE_LABELS[value]}
            </button>
          ))}
          <button
            type="button"
            className="reader-tool"
            onClick={() => setDirection((value) => (value === "rtl" ? "ltr" : "rtl"))}
            aria-label={`Hướng đọc: ${direction === "rtl" ? "phải sang trái" : "trái sang phải"}`}
          >
            {direction === "rtl" ? "RTL ⇄" : "LTR ⇄"}
          </button>
          <button type="button" className="reader-tool" onClick={() => setZoom((z) => Math.max(0.5, +(z - 0.25).toFixed(2)))} aria-label="Thu nhỏ">
            <ZoomOut size={16} aria-hidden="true" />
          </button>
          <span>{Math.round(zoom * 100)}%</span>
          <button type="button" className="reader-tool" onClick={() => setZoom((z) => Math.min(3, +(z + 0.25).toFixed(2)))} aria-label="Phóng to">
            <ZoomIn size={16} aria-hidden="true" />
          </button>
          <button
            type="button"
            className="reader-tool"
            onClick={() => void toggleFullscreen()}
            aria-label={fullscreen ? "Thoát toàn màn hình" : "Toàn màn hình"}
          >
            {fullscreen ? <Minimize size={16} aria-hidden="true" /> : <Maximize size={16} aria-hidden="true" />}
          </button>
        </div>
      </header>

      <ReaderModes
        manifest={data}
        page={page}
        mode={mode}
        direction={direction}
        zoom={zoom}
        urls={urls}
        errors={errors}
        onRetry={retry}
        onForward={stepForward}
        onBackward={stepBack}
        onScrollPage={goTo}
      />

      <nav className="reader-pager" aria-label="Điều hướng trang">
        <button type="button" className="reader-tool" onClick={stepBack} disabled={page <= 0} aria-label="Trang trước">
          ← Trước
        </button>
        {data.prevChapterId && (
          <button type="button" className="reader-tool" onClick={() => navigate(`/read/${data.prevChapterId}`)} aria-label="Chương trước">
            〈 Chương trước
          </button>
        )}
        {data.nextChapterId && (
          <button type="button" className="reader-tool" onClick={() => navigate(`/read/${data.nextChapterId}`)} aria-label="Chương tiếp">
            Chương tiếp 〉
          </button>
        )}
        <button
          type="button"
          className="reader-tool"
          onClick={stepForward}
          disabled={page >= data.pageCount - 1}
          aria-label="Trang sau"
        >
          Sau →
        </button>
      </nav>
    </div>
  );
}
