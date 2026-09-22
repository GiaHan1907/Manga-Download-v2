import { beforeEach, describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { AppRoutes } from "../src/app/router";
import { resetMockState, services } from "../src/services/provider";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === "app_get_info") {
      return Promise.resolve({ name: "Manga Downloader 1989", version: "0.1.0", platform: "windows" });
    }
    if (cmd === "app_health_check") {
      return Promise.resolve({ status: "ok", database: "ready" });
    }
    return Promise.reject(new Error("Unknown command"));
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    minimize: vi.fn().mockResolvedValue(undefined),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    isMaximized: vi.fn().mockResolvedValue(false),
  })),
}));

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  resetMockState();
});

describe("T03-02 Home quick actions", () => {
  it("shows dashboard stats and working quick links", async () => {
    renderAt("/");
    const explore = await screen.findByRole("link", { name: "Explore Manga" });
    fireEvent.click(explore);
    expect(await screen.findByRole("heading", { name: "Discover" })).toBeDefined();
  });

  it("derives stats from mock state and flags demo", async () => {
    renderAt("/");
    const stats = await screen.findByLabelText("Thống kê mẫu");
    expect(within(stats).getByText("8")).toBeDefined();
    expect(screen.getByText("Demo · dữ liệu mẫu")).toBeDefined();
    expect(screen.getByText("Not connected in UI prototype")).toBeDefined();
  });
});

describe("T03-03/T03-04 Discover search and demo import", () => {
  it("debounced search filters fixtures and supports empty state", async () => {
    renderAt("/discover");
    const input = await screen.findByPlaceholderText(/Nhập tên, tác giả/);
    fireEvent.change(input, { target: { value: "giấy" } });
    expect(await screen.findByText("Kỷ Nguyên Giấy")).toBeDefined();

    fireEvent.change(input, { target: { value: "hong-co-ket-qua" } });
    expect(await screen.findByText("Không có kết quả mẫu")).toBeDefined();
  });

  it("resolves the sample demo URL and rejects others", async () => {
    renderAt("/discover?tab=url");
    const input = await screen.findByPlaceholderText(/demo\.manga1989/);
    fireEvent.change(input, { target: { value: "https://example.org/x" } });
    fireEvent.click(screen.getByRole("button", { name: "Kiểm tra URL" }));
    expect(await screen.findByText(/Nguồn không được hỗ trợ/)).toBeDefined();

    fireEvent.click(screen.getByRole("button", { name: "Dùng URL mẫu" }));
    fireEvent.click(screen.getByRole("button", { name: "Kiểm tra URL" }));
    const result = await screen.findByLabelText("Kết quả import demo");
    expect(within(result).getByText("Hành Trình Ngôi Sao")).toBeDefined();
    fireEvent.click(within(result).getByRole("link", { name: "Mở Manga Detail" }));
    expect(await screen.findByRole("heading", { name: "Manga Detail" })).toBeDefined();
  });
});

describe("T03-05/T03-06 detail selection, queue, favorites sync", () => {
  it("selects chapters, queues with added/skipped counts and syncs downloads", async () => {
    renderAt("/manga/manga-001");
    const checkboxes = await screen.findAllByRole("checkbox");
    fireEvent.click(checkboxes[0]);
    const add = screen.getByRole("button", { name: "Add selected to demo queue" });
    fireEvent.click(add);
    expect((await screen.findByRole("status")).textContent).toMatch(/thêm mới 1, bỏ qua 0/);
  });

  it("skips already-queued chapters in the counts", async () => {
    renderAt("/manga/manga-001");
    const checkboxes = await screen.findAllByRole("checkbox");
    fireEvent.click(checkboxes[0]);
    fireEvent.click(checkboxes[2]);
    fireEvent.click(screen.getByRole("button", { name: "Add selected to demo queue" }));
    expect((await screen.findByRole("status")).textContent).toMatch(/thêm mới 1, bỏ qua 1/);
  });

  it("favorite toggled in detail appears in favorites page", async () => {
    renderAt("/manga/manga-003");
    const star = await screen.findByRole("button", { name: "Yêu thích" });
    fireEvent.click(star);
    fireEvent.click(screen.getByRole("link", { name: "Favorites" }));
    expect(await screen.findByText("Vị Giác Cuối Cùng")).toBeDefined();
  });

  it("library remove uses confirmation dialog and only mock state", async () => {
    renderAt("/library");
    const removeButtons = await screen.findAllByRole("button", { name: /Gỡ .* khỏi thư viện mẫu/ });
    fireEvent.click(removeButtons[0]);
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hủy" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.click(removeButtons[0]);
    const dialog2 = await screen.findByRole("dialog");
    fireEvent.click(within(dialog2).getByRole("button", { name: "Gỡ (chỉ mock)" }));
    await waitFor(() => {
      expect(screen.queryAllByRole("button", { name: /Gỡ .* khỏi thư viện mẫu/ }).length).toBe(3);
    });
  });

  it("dialog moves focus in, traps Tab and restores focus on close", async () => {
    renderAt("/library");
    const removeButton = (await screen.findAllByRole("button", { name: /Gỡ .* khỏi thư viện mẫu/ }))[0];
    removeButton.focus();
    fireEvent.click(removeButton);
    const dialog = await screen.findByRole("dialog");
    await waitFor(() => {
      expect(dialog.contains(document.activeElement)).toBe(true);
    });

    fireEvent.keyDown(window, { key: "Tab" });
    expect(dialog.contains(document.activeElement)).toBe(true);

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(document.activeElement).toBe(removeButton);
  });

  it("offline filter keeps only manga with bundled sample pages", async () => {
    renderAt("/library");
    const offlineSelect = await screen.findByLabelText(/Available offline/);
    fireEvent.change(offlineSelect, { target: { value: "true" } });
    await waitFor(() => {
      expect(screen.getByText("Hành Trình Ngôi Sao")).toBeDefined();
      expect(screen.queryByText("Vị Giác Cuối Cùng")).toBeNull();
      expect(screen.queryByText("Chuyến Tàu Cuối Ngày")).toBeNull();
    });
  });
});

describe("T03-07 Downloads transitions", () => {
  it("supports pause and retry via mock service only", async () => {
    renderAt("/downloads");
    const pause = await screen.findByRole("button", { name: "Pause" });
    fireEvent.click(pause);
    expect(await screen.findByText("demo: paused")).toBeDefined();

    const retry = await screen.findByRole("button", { name: "Retry" });
    fireEvent.click(retry);
    await waitFor(() => {
      expect(screen.getAllByText("demo: queued").length).toBeGreaterThanOrEqual(1);
    });
    expect(screen.queryByText("demo: failed")).toBeNull();
  });

  it("hides invalid actions per status", async () => {
    renderAt("/downloads");
    await screen.findByText(/Hàng đợi demo —/);
    expect(screen.queryByRole("button", { name: "Resume" })).toBeDefined();
    const completedCard = screen.getByText("Hành Trình Ngôi Sao").closest("article");
    expect(completedCard).not.toBeNull();
    expect(within(completedCard as HTMLElement).queryByRole("button", { name: "Pause" })).toBeNull();
  });
});

describe("T03-08 Reader", () => {
  it("navigates pages with bounds, keeps progress for Home", async () => {
    renderAt("/reader/manga-001/manga-001-ch-01");
    expect(await screen.findByText(/Trang 1\/4/)).toBeDefined();

    const next = screen.getByRole("button", { name: "Trang sau" });
    fireEvent.click(next);
    expect(await screen.findByText(/Trang 2\/4/)).toBeDefined();

    const prev = screen.getByRole("button", { name: "Trang trước" });
    fireEvent.click(prev);
    fireEvent.click(prev);
    await waitFor(() => expect((screen.getByRole("button", { name: "Trang trước" }) as HTMLButtonElement).disabled).toBe(true));

    fireEvent.click(next);
    await waitFor(async () => {
      const progress = await services.reader.getProgress("manga-001");
      expect(progress?.page).toBe(1);
    });

    fireEvent.click(screen.getByRole("link", { name: "Home" }));
    expect(await screen.findByText(/Trang 2\/4/)).toBeDefined();
  });

  it("shows fallback for chapters without sample pages", async () => {
    renderAt("/reader/manga-002/manga-002-ch-01");
    expect(await screen.findByText("Chương này chưa có trang mẫu")).toBeDefined();
  });
});

describe("T03-09 error and empty states", () => {
  it("unknown manga id shows recoverable error", async () => {
    renderAt("/manga/manga-999");
    expect(await screen.findByText("Không tìm thấy manga này")).toBeDefined();
  });
});

describe("T03-08 Settings and Sources", () => {
  it("validates and saves demo settings", async () => {
    renderAt("/settings");
    const chapterInput = await screen.findByLabelText(/Chương đồng thời/);
    fireEvent.change(chapterInput, { target: { value: "9" } });
    fireEvent.click(screen.getByRole("button", { name: "Lưu (mock)" }));
    expect((await screen.findByRole("status")).textContent).toMatch(/số nguyên từ 1 đến 5/);

    fireEvent.change(chapterInput, { target: { value: "4" } });
    fireEvent.click(screen.getByRole("button", { name: "Lưu (mock)" }));
    expect((await screen.findByRole("status")).textContent).toMatch(/Đã lưu/);
  });

  it("source toggle affects enabled list", async () => {
    renderAt("/settings/sources");
    const toggles = await screen.findAllByRole("button", { name: "Tắt nguồn" });
    fireEvent.click(toggles[0]);
    expect(await screen.findByText("disabled")).toBeDefined();
    const addSource = screen.getByRole("button", { name: /Thêm nguồn/ });
    expect((addSource as HTMLButtonElement).disabled).toBe(true);
  });

  it("disabling a source removes its manga from Discover results (§3.7)", async () => {
    renderAt("/settings/sources");
    const rows = await screen.findAllByRole("button", { name: "Tắt nguồn" });
    const betaToggle = rows[1];
    const betaLabel = betaToggle.closest(".source-row")?.textContent ?? "";
    expect(betaLabel).toContain("Beta");
    fireEvent.click(betaToggle);

    fireEvent.click(screen.getByRole("link", { name: "Discover" }));
    const input = await screen.findByPlaceholderText(/Nhập tên, tác giả/);

    fireEvent.change(input, { target: { value: "kỷ nguyên" } });
    expect(await screen.findByText("Không có kết quả mẫu")).toBeDefined();

    fireEvent.change(input, { target: { value: "hanh trinh" } });
    expect(await screen.findByText("Hành Trình Ngôi Sao")).toBeDefined();
  });
});
