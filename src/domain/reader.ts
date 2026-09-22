export type ReaderMode = "single" | "double" | "vertical";
export type ReadingDirection = "ltr" | "rtl";

export interface ReaderPage {
  id: string;
  chapterId: string;
  index: number;
  src: string;
  alt: string;
}

export interface ReadingProgress {
  mangaId: string;
  chapterId: string;
  page: number;
}

export const ZOOM_MIN = 0.5;
export const ZOOM_MAX = 3;
export const ZOOM_STEP = 0.25;

export function clampZoom(value: number): number {
  return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(value * 100) / 100));
}

export function clampPage(page: number, total: number): number {
  if (total <= 0) {
    return 0;
  }
  return Math.min(total - 1, Math.max(0, page));
}
