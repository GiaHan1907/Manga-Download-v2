import type { ReaderMode } from "../../domain/reader";

/// Pure reader layout/navigation logic (unit-testable, DOM-free). Page indices
/// are always LOGICAL (0 = first page); direction only changes nav semantics.

export type Spread = { primary: number; secondary: number | null };

/// Group pages into double-page spreads. `firstSingle` shows page 1 alone
/// (common manga cover convention), then pairs (2,3), (4,5)…
export function buildSpreads(pageCount: number, firstSingle: boolean): Spread[] {
  if (pageCount <= 0) {
    return [];
  }
  const spreads: Spread[] = [];
  let index = 0;
  if (firstSingle) {
    spreads.push({ primary: 0, secondary: null });
    index = 1;
  }
  for (; index < pageCount; index += 2) {
    const secondary = index + 1 < pageCount ? index + 1 : null;
    spreads.push({ primary: index, secondary });
  }
  return spreads;
}

/// Which spread shows a given logical page.
export function spreadIndexForPage(spreads: Spread[], page: number): number {
  const found = spreads.findIndex(
    (spread) => spread.primary === page || spread.secondary === page,
  );
  return found < 0 ? 0 : found;
}

/// Advance/rewind by one logical page, clamped. RTL is a *display* concern:
/// "next page" still means further into the story, so this is direction-free
/// by design (tested explicitly in the UI layer).
export function clampPage(page: number, pageCount: number): number {
  if (pageCount <= 0) {
    return 0;
  }
  return Math.min(pageCount - 1, Math.max(0, page));
}

export function nextPage(page: number, pageCount: number): number {
  return Math.min(pageCount - 1, page + 1);
}

export function prevPage(page: number): number {
  return Math.max(0, page - 1);
}

/// Page rendered for a spread given reading direction: in RTL the visual
/// right page is the earlier one, so a spread reads secondary|primary.
export function spreadOrder(spread: Spread, direction: "ltr" | "rtl"): number[] {
  const items = [spread.primary, spread.secondary].filter(
    (value): value is number => value !== null,
  );
  return direction === "rtl" ? [...items].reverse() : items;
}

/// Bounded prefetch window: pages to keep loaded around the current page so
/// memory stays flat for 300-page chapters.
export function prefetchWindow(page: number, pageCount: number, radius: number): number[] {
  const start = Math.max(0, page - radius);
  const end = Math.min(pageCount - 1, page + radius);
  const result: number[] = [];
  for (let index = start; index <= end; index += 1) {
    result.push(index);
  }
  return result;
}

export function progressPercent(page: number, pageCount: number): number {
  if (pageCount <= 0) {
    return 0;
  }
  return Math.min(100, Math.round(((page + 1) / pageCount) * 100));
}

export type { ReaderMode };
