import { Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useDebouncedValue } from "../discover/hooks/useDebouncedValue";
import { libraryApi } from "../../services/library.api";
import type { LibraryMangaDto } from "../../types/library";

const RESULT_LIMIT = 6;

type Item =
  | { kind: "action"; id: string; label: string; path: string }
  | { kind: "manga"; id: string; manga: LibraryMangaDto }
  | { kind: "online"; id: string; query: string };

type ActionItem = { kind: "action"; id: string; label: string; path: string };

const ACTIONS: ActionItem[] = [
  { kind: "action", id: "a-home", label: "Home", path: "/" },
  { kind: "action", id: "a-discover", label: "Discover (nguồn thật)", path: "/discover" },
  { kind: "action", id: "a-library", label: "Library", path: "/library" },
  { kind: "action", id: "a-favorites", label: "Favorites", path: "/favorites" },
  { kind: "action", id: "a-downloads", label: "Downloads", path: "/downloads" },
  { kind: "action", id: "a-settings", label: "Settings", path: "/settings" },
  { kind: "action", id: "a-sources", label: "Sources", path: "/settings/sources" },
];

export const OPEN_PALETTE_EVENT = "open-command-palette";

/// Phase 08 global palette: Ctrl+K (or Topbar button) opens a keyboard-only
/// dialog over LOCAL data. Remote search is never implicit — the explicit
/// "Search online" item hands over to Discover where the user triggers it.
/// Titles render as React text nodes (metadata never becomes HTML).
export default function CommandPalette() {
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [manga, setManga] = useState<LibraryMangaDto[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  const debounced = useDebouncedValue(query.trim(), 200);

  function openPalette() {
    openerRef.current = document.activeElement as HTMLElement | null;
    setQuery("");
    setManga([]);
    setLoadError(null);
    setOpen(true);
  }

  function closePalette() {
    setOpen(false);
    setQuery("");
    openerRef.current?.focus?.();
    openerRef.current = null;
  }

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && (event.key === "k" || event.key === "K")) {
        event.preventDefault();
        if (open) {
          closePalette();
        } else {
          openPalette();
        }
      } else if (open && event.key === "Escape") {
        // Global escape: works even if focus has drifted out of the dialog.
        event.preventDefault();
        closePalette();
      } else if (open && event.key === "Tab") {
        // Focus trap: the dialog's only tabbable control is the input;
        // Tab must not scroll behind the overlay.
        event.preventDefault();
        inputRef.current?.focus();
      }
    }
    window.addEventListener("keydown", onKey);
    window.addEventListener(OPEN_PALETTE_EVENT, openPalette);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener(OPEN_PALETTE_EVENT, openPalette);
    };
    // open only gates the toggle direction; handlers are rebuilt per render.
  });

  useEffect(() => {
    if (open) {
      inputRef.current?.focus();
    }
  }, [open]);

  // Local search only when open + non-empty query; bounded and debounced.
  useEffect(() => {
    let alive = true;
    // Deferred so no setState happens synchronously in the effect body
    // (same convention as the download queue hook).
    void Promise.resolve().then(() => {
      if (!alive) {
        return;
      }
      if (!open || debounced.length === 0) {
        setManga([]);
        setLoadError(null);
        setLoading(false);
        return;
      }
      setLoading(true);
      libraryApi
        .list({ query: debounced, limit: RESULT_LIMIT, offset: 0 })
        .then((page) => {
          if (alive) {
            setManga(page.items);
            setLoadError(null);
          }
        })
        .catch((error: unknown) => {
          if (alive) {
            setManga([]);
            setLoadError(normalizeIpcError(error).message);
          }
        })
        .finally(() => {
          if (alive) {
            setLoading(false);
          }
        });
    });
    return () => {
      alive = false;
    };
  }, [open, debounced]);

  const trimmed = query.trim();
  const items = useMemo<Item[]>(() => {
    if (trimmed.length === 0) {
      return ACTIONS;
    }
    const mangaItems: Item[] = manga.map((entry) => ({
      kind: "manga",
      id: `m-${entry.id}`,
      manga: entry,
    }));
    const lower = trimmed.toLowerCase();
    const actions = ACTIONS.filter((item) => item.label.toLowerCase().includes(lower));
    const online: Item[] = [{ kind: "online", id: "online", query: trimmed }];
    return [...mangaItems, ...online, ...actions];
  }, [trimmed, manga]);

  // Clamp instead of a reset-effect: the list changes while typing, and
  // setState-in-effect is exactly the cascade this codebase avoids.
  const safeSelected = Math.min(selected, Math.max(0, items.length - 1));

  function choose(item: Item) {
    closePalette();
    if (item.kind === "action") {
      navigate(item.path);
    } else if (item.kind === "manga") {
      navigate(`/library/${item.manga.id}`);
    } else {
      navigate(`/discover?q=${encodeURIComponent(item.query)}`);
    }
  }

  function onKeyDown(event: React.KeyboardEvent) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelected((value) => Math.min(items.length - 1, value + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelected((value) => Math.max(0, value - 1));
    } else if (event.key === "Enter" && items[safeSelected]) {
      event.preventDefault();
      choose(items[safeSelected]);
    } else if (event.key === "Escape") {
      event.preventDefault();
      closePalette();
    }
  }

  if (!open) {
    return null;
  }

  return (
    <div className="palette-overlay" onClick={closePalette} data-testid="palette-overlay">
      <div
        className="palette"
        role="dialog"
        aria-modal="true"
        aria-label="Tìm kiếm và lệnh nhanh"
        onClick={(event) => event.stopPropagation()}
        onKeyDown={onKeyDown}
      >
        <div className="palette__input-row">
          <Search size={16} aria-hidden="true" />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Tìm truyện cục bộ hoặc lệnh… (Esc đóng)"
            aria-label="Nhập từ khóa tìm kiếm cục bộ"
          />
        </div>
        {loading && <p className="muted palette__status">Đang tìm trong library…</p>}
        {loadError && (
          <p className="muted palette__status" role="alert">
            {loadError}
          </p>
        )}
        {!loading && trimmed.length === 0 && (
          <p className="muted palette__status">Lệnh nhanh — điều hướng trong app.</p>
        )}
        {items.length === 0 && (
          <p className="muted palette__status">Không có kết quả cục bộ cho “{trimmed}”.</p>
        )}
        <ul className="palette__list" role="listbox" aria-label="Kết quả">
          {items.map((item, index) => (
            <li
              key={item.id}
              role="option"
              aria-selected={index === safeSelected}
              className={`palette__item${index === safeSelected ? " palette__item--active" : ""}`}
              onMouseEnter={() => setSelected(index)}
              onClick={() => choose(item)}
            >
              {item.kind === "action" && <span>{item.label}</span>}
              {item.kind === "manga" && (
                <span>
                  {item.manga.title}{" "}
                  <span className="muted">
                    · {item.manga.downloadedChapters}/{item.manga.totalChapters} offline
                  </span>
                </span>
              )}
              {item.kind === "online" && (
                <span>Tìm online “{item.query}” trong Discover</span>
              )}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
