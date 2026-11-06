import { describe, expect, it } from "vitest";
import { canCommitImport, selectedCsvPath, statementKind } from "./importFlow";

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

  it("only imports a PDF whose rows agree with the printed balances", () => {
    const check = { openingBalancePaise: 100, closingBalancePaise: 50, depositsTotalPaise: 0, withdrawalsTotalPaise: 50, totalsMatch: true, everyRowBalances: true };
    const pdf = (patch: object) => ({ rowsNew: 2, statementCheck: { ...check, ...patch } }) as never;
    expect(canCommitImport("a", "/s.pdf", pdf({}))).toBe(true);
    expect(canCommitImport("a", "/s.pdf", pdf({ everyRowBalances: false }))).toBe(false);
    expect(canCommitImport("a", "/s.pdf", pdf({ totalsMatch: false }))).toBe(false);
    expect(canCommitImport("a", "/s.pdf", pdf({ totalsMatch: false, closingBalancePaise: null }))).toBe(true);
  });

  it("tells statement types apart by extension, ignoring case", () => {
    expect(statementKind("/a/Statement.PDF")).toBe("pdf");
    expect(statementKind("/a/b.xlsx")).toBe("xlsx");
    expect(statementKind("/a/b.csv")).toBe("csv");
  });
});
