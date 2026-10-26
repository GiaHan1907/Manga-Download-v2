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
import { pushToast } from "../../components/ui/toastStore";
import { normalizeIpcError, openChapterFile } from "../../shared/api/desktop";
import type { ReaderManifestDto } from "../../types/library";
import { localServices } from "../../services/provider";
import { settingsApi } from "../../services/settings.api";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { clampPage, nextPage, prevPage } from "./readerLogic";
import { usePageImages } from "./usePageImages";
import { useReaderProgress } from "./useReaderProgress";
import ReaderModes from "./ReaderModes";
import ReaderProgressRing from "./components/ReaderProgressRing";

type ReaderMode = "single" | "double" | "vertical";
type Direction = "ltr" | "rtl";
type ReaderTheme = "default" | "sepia" | "night";

const MODES: ReaderMode[] = ["single", "double", "vertical"];
const MODE_LABELS: Record<ReaderMode, string> = {
  single: "Một trang",
  double: "Hai trang",
  vertical: "Cuộn dọc",
};
const THEME_LABELS: Record<ReaderTheme, string> = {
  default: "Nền tối mặc định",
  sepia: "Nền giấy ấm",
  night: "Nền đen sâu",
};
const THEME_ORDER: ReaderTheme[] = ["default", "sepia", "night"];
const CHROME_HIDE_AFTER_MS = 2500;

function readStoredTheme(): ReaderTheme {
  try {
    const raw = window.localStorage.getItem("reader.theme");
    return THEME_ORDER.includes(raw as ReaderTheme) ? (raw as ReaderTheme) : "default";
  } catch {
    return "default";
  }
}

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

/// Navigating between /read/:chapterId routes reuses the element, so the
/// session is keyed by chapter: every piece of per-chapter view state (page,
/// zoom, resume flag, blob cache) remounts clean instead of leaking stale.
export default function ReadPage() {
  const { chapterId = "" } = useParams();
  return <ReaderSession key={chapterId} chapterId={chapterId} />;
}

function ReaderSession({ chapterId }: { chapterId: string }) {
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
  const [theme, setTheme] = useState<ReaderTheme>(readStoredTheme);
  const [fullscreen, setFullscreen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const headerRef = useRef<HTMLElement>(null);
  const fullscreenBtnRef = useRef<HTMLButtonElement>(null);
  const hideTimer = useRef<number | null>(null);
  const initialized = useRef(false);
  // Last scroll-derived position (vertical mode) for progress persistence.
  const scrollInfo = useRef<{ page: number; fraction: number } | null>(null);
  const pendingResumeFraction = useRef<number | null>(null);

  // Escape hatch from the "không mở được chương" screen: a PDF volume (the
  // reader only draws page images) or a folder opens in the OS handler. Rust
  // resolves and validates the path; failures surface as a toast.
  async function openExternally() {
    try {
      await openChapterFile({ chapterId });
    } catch (error) {
      pushToast(normalizeIpcError(error).message);
    }
  }

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
  const startScrollOffset = manifest.state === "success" ? manifest.data.startScrollOffset : null;
  const { urls, errors, retry } = usePageImages(chapterId, page, mode, pageCount, fetchPage);

  // Once the manifest resolves, jump to the stored resume position.
  useEffect(() => {
    if (manifest.state === "success" && !initialized.current) {
      initialized.current = true;
      if (startPage > 0 && startScrollOffset) {
        pendingResumeFraction.current = startScrollOffset;
      }
      setPage(clampPage(startPage, pageCount || 1));
    }
  }, [manifest.state, startPage, startScrollOffset, pageCount]);

  const goTo = useCallback(
    (candidate: number) => setPage(clampPage(candidate, pageCount || 1)),
    [pageCount],
  );
  const handleScrollPage = useCallback(
    (candidate: number, fraction: number) => {
      scrollInfo.current = { page: candidate, fraction };
      goTo(candidate);
    },
    [goTo],
  );
  const consumeResumeFraction = useCallback(() => {
    const value = pendingResumeFraction.current;
    pendingResumeFraction.current = null;
    return value;
  }, []);
  const stepForward = useCallback(() => {
    setPage((current) => (pageCount ? nextPage(current, pageCount) : current));
  }, [pageCount]);
  const stepBack = useCallback(() => {
    setPage((current) => (pageCount ? prevPage(current) : current));
  }, [pageCount]);

  useReaderProgress(manifest, page, scrollInfo);

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
    hideTimer.current = window.setTimeout(() => {
      // Keep the chrome while the user is operating its controls (keyboard
      // focus inside toolbar) — hiding mid-interaction would trap focus.
      const active = document.activeElement;
      if (headerRef.current && active && headerRef.current.contains(active)) {
        return;
      }
      setChromeVisible(false);
    }, CHROME_HIDE_AFTER_MS);
  }

  useEffect(() => {
    try {
      window.localStorage.setItem("reader.theme", theme);
    } catch {
      // Storage unavailable (private mode) — theme stays session-only.
    }
  }, [theme]);

  useEffect(() => {
    return () => {
      if (hideTimer.current) {
        window.clearTimeout(hideTimer.current);
      }
    };
  }, []);

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
    let wasFullscreen = Boolean(document.fullscreenElement);
    const sync = () => {
      const now = Boolean(document.fullscreenElement);
      setFullscreen(now);
      // OS/Esc exits fullscreen without our button — hand focus back so
      // keyboard users don't land on <body>.
      if (wasFullscreen && !now && document.activeElement === document.body) {
        fullscreenBtnRef.current?.focus();
      }
      wasFullscreen = now;
    };
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
          <>
            {/* The one failure this screen can fix itself: a PDF volume (or any
                folder the reader cannot draw) is readable in the OS handler. */}
            <button
              type="button"
              className="btn btn--secondary"
              onClick={() => void openExternally()}
            >
              Mở bằng ứng dụng ngoài
            </button>
            <button type="button" className="btn btn--secondary" onClick={() => navigate(-1)}>
              Quay lại
            </button>
          </>
        }
      />
    );
  }
  const data = manifest.data;

  return (
    <div
      ref={containerRef}
      className={`reader-shell${fullscreen || document.fullscreenElement ? " reader-shell--full" : ""}`}
      data-reader-theme={theme}
      onMouseMove={toggleChromeWithActivity}
      onClick={toggleChromeWithActivity}
      data-testid="reader-shell"
    >
      <header
        ref={headerRef}
        className={`reader-toolbar${chromeVisible ? "" : " reader-toolbar--hidden"}`}
      >
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
            onClick={() => setTheme((value) => THEME_ORDER[(THEME_ORDER.indexOf(value) + 1) % THEME_ORDER.length])}
            aria-label={`Nền đọc: ${THEME_LABELS[theme]} — bấm để đổi`}
          >
            Nền: {THEME_LABELS[theme]}
          </button>
          <button
            ref={fullscreenBtnRef}
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
        onScrollPage={handleScrollPage}
        consumeResumeFraction={consumeResumeFraction}
      />

      <ReaderProgressRing page={page} pageCount={data.pageCount} />

      <nav
        className={`reader-pager${chromeVisible ? "" : " reader-pager--hidden"}`}
        aria-label="Điều hướng trang"
      >
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
