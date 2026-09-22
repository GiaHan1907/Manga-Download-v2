import type { ChapterSummary, MangaDetail, MangaSource, MangaSummary } from "../../domain/manga";
import type { DemoDownloadTask } from "../../domain/download";
import type { ReaderPage } from "../../domain/reader";
import coverUrl from "../../assets/sample-reader/sample-cover.svg";
import page1 from "../../assets/sample-reader/sample-page-1.svg";
import page2 from "../../assets/sample-reader/sample-page-2.svg";
import page3 from "../../assets/sample-reader/sample-page-3.svg";
import page4 from "../../assets/sample-reader/sample-page-4.svg";
import page5 from "../../assets/sample-reader/sample-page-5.svg";

// Deterministic demo catalog. Fixed IDs and ISO timestamps only — no Math.random()
// and no current-time reads, so seeds stay stable and tests are reproducible.

export const DEMO_SOURCES: MangaSource[] = [
  { id: "src-alpha", label: "Demo Source Alpha", demo: true, enabledByDefault: true },
  { id: "src-beta", label: "Demo Source Beta", demo: true, enabledByDefault: true },
];

type MangaSeed = {
  id: string;
  sourceId: string;
  sourceMangaId: string;
  title: string;
  author: string;
  genres: string[];
  status: MangaSummary["status"];
  description: string;
  chapterCount: number;
};

const MANGA_SEEDS: MangaSeed[] = [
  { id: "manga-001", sourceId: "src-alpha", sourceMangaId: "alpha-101", title: "Hành Trình Ngôi Sao", author: "Aoi Haruka", genres: ["phiêu lưu", "khoa cử", "chính kịch"], status: "ongoing", description: "Nhóm bạn trẻ băng qua các tinh cầu đã tắt để tìm lại ngọn lửa đầu tiên của nhân loại. Một câu chuyện dài về tình bạn và ký ức.", chapterCount: 5 },
  { id: "manga-002", sourceId: "src-alpha", sourceMangaId: "alpha-102", title: "Thư Viện Nửa Đêm", author: "Trần Minh", genres: ["kinh dị", "bí ẩn"], status: "completed", description: "Mỗi đêm, một thủ thư nhận những cuốn sách chưa từng được viết. Đọc chúng nghĩa là sống lại cái kết của người khác.", chapterCount: 4 },
  { id: "manga-003", sourceId: "src-beta", sourceMangaId: "beta-201", title: "Vị Giác Cuối Cùng", author: "Lê An Nhiên", genres: ["đời thường", "ẩm thực"], status: "ongoing", description: "Đầu bếp mất vị giác quay về quán ăn của bà ngoại và học lại cách nếm cuộc sống qua từng món ăn giản dị.", chapterCount: 6 },
  { id: "manga-004", sourceId: "src-beta", sourceMangaId: "beta-202", title: "Búp Bê Biết Mơ", author: "Sato Kenji", genres: ["fantasy", "tâm lý"], status: "unknown", description: "Một xưởng đồ chơi cổ sản xuất con búp bê có khả năng mơ hộ chủ nhân — nhưng giấc mơ của ai đang rò rỉ?", chapterCount: 3 },
  { id: "manga-005", sourceId: "src-alpha", sourceMangaId: "alpha-103", title: "Thành Phố Không Ngủ Dưới Biển", author: "Ngô Hải Đăng", genres: ["khoa cử", "chính kịch"], status: "completed", description: "Dưới rãnh sâu, một thành phố vận hành bằng ánh sáng sinh học đang chìm dần vào bóng tối của chính nó.", chapterCount: 5 },
  { id: "manga-006", sourceId: "src-beta", sourceMangaId: "beta-203", title: "Kỷ Nguyên Giấy", author: "Mori Yuki", genres: ["lịch sử", "giả tưởng"], status: "ongoing", description: "Khi mực mực biến mất khỏi thế giới, một thợ làm giấy trở thành người cuối cùng còn khả năng lưu giữ lịch sử.", chapterCount: 4 },
  { id: "manga-007", sourceId: "src-alpha", sourceMangaId: "alpha-104", title: "Chuyến Tàu Cuối Ngày", author: "Phạm Đức", genres: ["chữa lành", "đời thường"], status: "completed", description: "Mỗi tối, một toa tàu chỉ đón những hành khách vừa quyết định từ bỏ — và một người dẫn đường không bao giờ xuống.", chapterCount: 7 },
  { id: "manga-008", sourceId: "src-beta", sourceMangaId: "beta-204", title: "Khu Vườn Của Những Câu Hỏi Bỏ Ngỏ Rất Dài Chưa Từng Được Giải Đáp", author: "Anonymous", genres: ["triết lý", "fantasy"], status: "ongoing", description: "Một khu vườn mọc lên từ mọi câu hỏi bị bỏ giữa chừng; người làm vườn đang tìm hạt giống của câu hỏi đầu tiên.", chapterCount: 3 },
];

function buildChapters(seed: MangaSeed): ChapterSummary[] {
  return Array.from({ length: seed.chapterCount }, (_, index) => {
    const number = index + 1;
    const id = `${seed.id}-ch-${String(number).padStart(2, "0")}`;
    const hasSamplePages = seed.id === "manga-001" && number <= 2;
    return {
      id,
      mangaId: seed.id,
      title: `Chương ${number}`,
      number,
      pageCount: hasSamplePages ? (number === 1 ? 4 : 3) : undefined,
      hasSamplePages,
    } satisfies ChapterSummary;
  });
}

const CHAPTERS_BY_MANGA: Record<string, ChapterSummary[]> = Object.fromEntries(
  MANGA_SEEDS.map((seed) => [seed.id, buildChapters(seed)]),
);

const DETAILS_BY_ID: Record<string, MangaDetail> = Object.fromEntries(
  MANGA_SEEDS.map((seed) => [
    seed.id,
    {
      id: seed.id,
      sourceId: seed.sourceId,
      sourceMangaId: seed.sourceMangaId,
      title: seed.title,
      author: seed.author,
      coverAsset: coverUrl,
      genres: [...seed.genres],
      status: seed.status,
      description: seed.description,
      chapters: CHAPTERS_BY_MANGA[seed.id],
    } satisfies MangaDetail,
  ]),
);

export const MANGA_SUMMARIES: MangaSummary[] = MANGA_SEEDS.map((seed) => ({
  id: seed.id,
  sourceId: seed.sourceId,
  sourceMangaId: seed.sourceMangaId,
  title: seed.title,
  author: seed.author,
  coverAsset: coverUrl,
  genres: [...seed.genres],
  status: seed.status,
}));

export const ALL_CHAPTERS: ChapterSummary[] = Object.values(CHAPTERS_BY_MANGA).flat();

export function getMangaDetail(mangaId: string): MangaDetail | undefined {
  return DETAILS_BY_ID[mangaId];
}

export function getMangaSummary(mangaId: string): MangaSummary | undefined {
  return MANGA_SUMMARIES.find((manga) => manga.id === mangaId);
}

export function getChapters(mangaId: string): ChapterSummary[] {
  return CHAPTERS_BY_MANGA[mangaId] ?? [];
}

export function getChapter(chapterId: string): ChapterSummary | undefined {
  return ALL_CHAPTERS.find((chapter) => chapter.id === chapterId);
}

const SAMPLE_PAGES_BY_CHAPTER: Record<string, string[]> = {
  "manga-001-ch-01": [page1, page2, page3, page4],
  "manga-001-ch-02": [page2, page3, page5],
};

export function getSamplePages(chapterId: string): ReaderPage[] {
  const sources = SAMPLE_PAGES_BY_CHAPTER[chapterId] ?? [];
  return sources.map((src, index) => ({
    id: `${chapterId}-page-${index + 1}`,
    chapterId,
    index,
    src,
    alt: `Trang mẫu ${index + 1} (ảnh nội bộ, không phải trang truyện thật)`,
  }));
}

// Seed library: manga already "added" in the sample library.
export const SEED_LIBRARY_IDS = ["manga-001", "manga-003", "manga-005", "manga-007"];
export const SEED_FAVORITE_IDS = ["manga-001", "manga-005"];

// Fixed "recently added" ordering (newest first) for the Home dashboard.
export const RECENT_ADDITION_ORDER = ["manga-007", "manga-005", "manga-003", "manga-001"];

export const SEED_TASKS: DemoDownloadTask[] = [
  { id: "task-001", mangaId: "manga-001", chapterId: "manga-001-ch-03", status: "completed", completedPages: 12, totalPages: 12, createdAt: "2026-09-20T09:10:00.000Z" },
  { id: "task-002", mangaId: "manga-002", chapterId: "manga-002-ch-01", status: "failed", completedPages: 5, totalPages: 14, createdAt: "2026-09-20T09:20:00.000Z" },
  { id: "task-003", mangaId: "manga-003", chapterId: "manga-003-ch-02", status: "queued", completedPages: 0, totalPages: 10, createdAt: "2026-09-20T09:30:00.000Z" },
  { id: "task-004", mangaId: "manga-006", chapterId: "manga-006-ch-01", status: "paused", completedPages: 4, totalPages: 10, createdAt: "2026-09-20T09:40:00.000Z" },
];

// Documented fictitious demo URL that resolves to a fixture manga (local text match only).
export const DEMO_IMPORT_URL = "https://demo.manga1989.example/manga/manga-001";
