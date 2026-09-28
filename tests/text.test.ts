import { describe, expect, it } from "vitest";
import { matchesQuery } from "../src/shared/lib/text";

describe("matchesQuery", () => {
  it("matches case- and diacritic-insensitively", () => {
    expect(matchesQuery("Chương 12 — Tập Đặc Biệt", "dac biet")).toBe(true);
    expect(matchesQuery("Ninja Loạn Thị", "NINJA")).toBe(true);
    expect(matchesQuery("Đầu Gấu", "dau gau")).toBe(true);
  });

  it("treats an empty or whitespace query as no filter", () => {
    expect(matchesQuery("Bất kỳ", "   ")).toBe(true);
  });

  it("rejects non-matches", () => {
    expect(matchesQuery("Chương 1", "chapter 2")).toBe(false);
  });
});
