import { useEffect, useRef } from "react";
import type { ReaderManifestDto } from "../../types/library";
import { buildSpreads, spreadIndexForPage, spreadOrder } from "./readerLogic";

type Props = {
  manifest: ReaderManifestDto;
  page: number;
  mode: "single" | "double" | "vertical";
  direction: "ltr" | "rtl";
  zoom: number;
  urls: Record<number, string>;
  errors: Record<number, string | undefined>;
  onRetry: (index: number) => void;
  onForward: () => void;
  onBackward: () => void;
  onScrollPage?: (page: number) => void;
};

function PageSlot({
  index,
  manifest,
  url,
  error,
  onRetry,
  fitWidth,
}: {
  index: number;
  manifest: ReaderManifestDto;
  url?: string;
  error?: string;
  onRetry: (index: number) => void;
  fitWidth: boolean;
}) {
  const info = manifest.pages[index];
  const ratio = info?.width && info?.height ? info.height / info.width : 1.47;
  if (error) {
    return (
      <div className="reader-page reader-page--error" role="group" aria-label={`Lỗi trang ${index + 1}`}>
        <span>Trang {index + 1}: {error}</span>
        <button type="button" className="reader-tool" onClick={() => onRetry(index)}>
          Thử lại
        </button>
      </div>
    );
  }
  if (!url) {
    return (
      <div
        className="reader-page reader-page--loading"
        style={{ aspectRatio: `${ratio}` }}
        aria-label={`Đang tải trang ${index + 1}`}
      />
    );
  }
  return (
    <img
      className="reader-page__img"
      src={url}
      alt={`Trang ${index + 1}`}
      draggable={false}
      style={fitWidth ? { width: "100%", height: "auto" } : { height: "100%", width: "auto" }}
    />
  );
}

export default function ReaderModes(props: Props) {
  const { manifest, page, mode, direction, zoom, urls, errors, onRetry, onForward, onBackward, onScrollPage } = props;
  const scrollRef = useRef<HTMLDivElement>(null);

  // Vertical: report the topmost visible page for progress persistence.
  useEffect(() => {
    if (mode !== "vertical" || !onScrollPage) {
      return;
    }
    const container = scrollRef.current;
    if (!container || typeof IntersectionObserver === "undefined") {
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((entry) => entry.isIntersecting)
          .sort((a, b) => Number(a.target.getAttribute("data-page") ?? 0) - Number(b.target.getAttribute("data-page") ?? 0));
        const first = visible[0];
        if (first) {
          onScrollPage(Number(first.target.getAttribute("data-page") ?? 0));
        }
      },
      { root: container, threshold: 0.1 },
    );
    container.querySelectorAll("[data-page]").forEach((node) => observer.observe(node));
    return () => observer.disconnect();
  }, [mode, onScrollPage, manifest.chapterId]);

  if (mode === "vertical") {
    return (
      <div ref={scrollRef} className="reader-viewport reader-viewport--vertical" data-testid="reader-vertical">
        <div className="reader-column" style={{ width: `min(100%, ${Math.round(zoom * 720)}px)` }}>
          {manifest.pages.map((info) => (
            <div key={info.index} data-page={info.index} className="reader-page" style={{ marginBottom: "var(--space-2)" }}>
              <PageSlot
                index={info.index}
                manifest={manifest}
                url={urls[info.index]}
                error={errors[info.index]}
                onRetry={onRetry}
                fitWidth
              />
            </div>
          ))}
        </div>
      </div>
    );
  }

  if (mode === "double") {
    const spreads = buildSpreads(manifest.pageCount, true);
    const current = spreadIndexForPage(spreads, page);
    const spread = spreads[current] ?? { primary: page, secondary: null };
    const order = spreadOrder(spread, direction);
    return (
      <div
        className="reader-viewport reader-viewport--double"
        data-testid="reader-double"
        onClick={onForward}
        dir={direction}
      >
        <div className="reader-spread" style={{ transform: `scale(${zoom})` }}>
          {order.map((index) => (
            <PageSlot
              key={index}
              index={index}
              manifest={manifest}
              url={urls[index]}
              error={errors[index]}
              onRetry={onRetry}
              fitWidth={false}
            />
          ))}
          {spread.secondary === null && <div className="reader-page reader-page--empty" aria-hidden="true" />}
        </div>
      </div>
    );
  }

  return (
    <div
      className="reader-viewport reader-viewport--single"
      data-testid="reader-single"
      onClick={(event) => {
        const target = event.currentTarget.getBoundingClientRect();
        const inRightHalf = event.clientX > target.left + target.width / 2;
        const rtlForward = direction === "rtl" ? !inRightHalf : inRightHalf;
        if (rtlForward) {
          onForward();
        } else {
          onBackward();
        }
      }}
    >
      <div className="reader-page reader-page--single" style={{ transform: `scale(${zoom})` }}>
        <PageSlot
          index={page}
          manifest={manifest}
          url={urls[page]}
          error={errors[page]}
          onRetry={onRetry}
          fitWidth={false}
        />
      </div>
    </div>
  );
}
