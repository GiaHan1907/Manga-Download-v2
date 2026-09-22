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
  /** Vertical mode reports the topmost visible page AND the fraction (0..1)
   *  the viewport is scrolled into it — used for approximate resume. */
  onScrollPage?: (page: number, fraction: number) => void;
  /** One-shot: fraction stored with the resume position, applied after the
   *  programmatic scroll to the resumed page. */
  consumeResumeFraction?: () => number | null;
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
  const { manifest, page, mode, direction, zoom, urls, errors, onRetry, onForward, onBackward, onScrollPage, consumeResumeFraction } = props;
  const scrollRef = useRef<HTMLDivElement>(null);
  // Page most recently derived from user scrolling — the programmatic-scroll
  // effect must not fight it (that would snap the view on every scroll tick).
  const scrollReported = useRef<number | null>(null);

  // Vertical: report topmost visible page + fraction scrolled into it.
  function reportScroll() {
    const container = scrollRef.current;
    if (!container || !onScrollPage) {
      return;
    }
    const nodes = Array.from(container.querySelectorAll<HTMLElement>("[data-page]"));
    let current: HTMLElement | null = null;
    for (const node of nodes) {
      if (node.offsetTop <= container.scrollTop + 1) {
        current = node;
      } else {
        break;
      }
    }
    if (!current) {
      return;
    }
    const reported = Number(current.dataset.page ?? 0);
    const fraction = Math.min(
      1,
      Math.max(0, (container.scrollTop - current.offsetTop) / (current.clientHeight || 1)),
    );
    scrollReported.current = reported;
    onScrollPage(reported, fraction);
  }

  // Programmatic navigation (keys, pager buttons, resume): scroll the page
  // into view. Scroll-reported pages are skipped — the user is already there.
  useEffect(() => {
    if (mode !== "vertical") {
      return;
    }
    if (scrollReported.current === page) {
      return;
    }
    const container = scrollRef.current;
    const element = container?.querySelector<HTMLElement>(`[data-page="${page}"]`);
    if (!container || !element || typeof element.scrollIntoView !== "function") {
      return;
    }
    element.scrollIntoView({ block: "start" });
    const fraction = consumeResumeFraction?.();
    if (fraction) {
      container.scrollTop += fraction * (element.clientHeight || 0);
    }
  }, [page, mode, consumeResumeFraction]);

  if (mode === "vertical") {
    return (
      <div
        ref={scrollRef}
        className="reader-viewport reader-viewport--vertical"
        data-testid="reader-vertical"
        onScroll={reportScroll}
      >
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
