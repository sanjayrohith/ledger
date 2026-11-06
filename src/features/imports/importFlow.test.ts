import { describe, expect, it } from "vitest";
import { canCommitImport, selectedCsvPath } from "./importFlow";

const preview = { validRows: 1, rowsNew: 1 } as never;
const allKnown = { validRows: 3, rowsNew: 0 } as never;

describe("CSV import confirmation", () => {
  it("requires an account, selected file, and at least one new row", () => {
    expect(canCommitImport("", "/synthetic.csv", preview)).toBe(false);
    expect(canCommitImport("account", "", preview)).toBe(false);
    expect(canCommitImport("account", "/synthetic.csv", null)).toBe(false);
    expect(canCommitImport("account", "/synthetic.csv", preview)).toBe(true);
    expect(canCommitImport("account", "/synthetic.csv", allKnown)).toBe(false);
  });

  it("treats a cancelled picker or multi-file selection as no selected CSV", () => {
    expect(selectedCsvPath(null)).toBeNull();
    expect(selectedCsvPath(["/synthetic-a.csv", "/synthetic-b.csv"])).toBeNull();
    expect(selectedCsvPath("/synthetic.csv")).toBe("/synthetic.csv");
  });
});
