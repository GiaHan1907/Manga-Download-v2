import { describe, it, expect } from "vitest";
import {
  buildSpreads,
  clampPage,
  nextPage,
  prevPage,
  prefetchWindow,
  progressPercent,
  spreadIndexForPage,
  spreadOrder,
} from "../src/features/read/readerLogic";

describe("T07 reader layout logic (pure)", () => {
  it("double-page pairing with first page alone", () => {
    const spreads = buildSpreads(5, true);
    // [1], [2,3], [4,5]
    expect(spreads).toEqual([
      { primary: 0, secondary: null },
      { primary: 1, secondary: 2 },
      { primary: 3, secondary: 4 },
    ]);
  });

  it("double-page pairing handles odd tail without firstSingle", () => {
    const spreads = buildSpreads(4, false);
    expect(spreads).toEqual([
      { primary: 0, secondary: 1 },
      { primary: 2, secondary: 3 },
    ]);
    const odd = buildSpreads(3, false);
    expect(odd[odd.length - 1]).toEqual({ primary: 2, secondary: null });
  });

  it("spreadIndexForPage maps a logical page to its spread", () => {
    const spreads = buildSpreads(6, true);
    expect(spreadIndexForPage(spreads, 0)).toBe(0);
    expect(spreadIndexForPage(spreads, 1)).toBe(1);
    expect(spreadIndexForPage(spreads, 2)).toBe(1);
    expect(spreadIndexForPage(spreads, 5)).toBe(3);
    expect(spreadIndexForPage(spreads, 4)).toBe(2);
  });

  it("RTL reverses visual spread order; LTR keeps it", () => {
    const spread = { primary: 2, secondary: 3 };
    expect(spreadOrder(spread, "ltr")).toEqual([2, 3]);
    expect(spreadOrder(spread, "rtl")).toEqual([3, 2]);
    // Single-page spread (cover) has nothing to reverse.
    expect(spreadOrder({ primary: 0, secondary: null }, "rtl")).toEqual([0]);
  });

  it("page nav is bounded and direction-independent (logical pages)", () => {
    expect(clampPage(-3, 5)).toBe(0);
    expect(clampPage(9, 5)).toBe(4);
    expect(clampPage(2, 0)).toBe(0);
    expect(nextPage(3, 5)).toBe(4);
    expect(nextPage(4, 5)).toBe(4);
    expect(prevPage(0)).toBe(0);
    expect(prevPage(2)).toBe(1);
  });

  it("prefetch window is bounded to the page count", () => {
    expect(prefetchWindow(0, 5, 1)).toEqual([0, 1]);
    expect(prefetchWindow(4, 5, 1)).toEqual([3, 4]);
    expect(prefetchWindow(2, 5, 1)).toEqual([1, 2, 3]);
    expect(prefetchWindow(2, 5, 9)).toEqual([0, 1, 2, 3, 4]);
  });

  it("progress percent clamps to a whole number", () => {
    expect(progressPercent(0, 4)).toBe(25);
    expect(progressPercent(3, 4)).toBe(100);
    expect(progressPercent(0, 0)).toBe(0);
  });
});
