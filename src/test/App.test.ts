import { describe, expect, it } from "vitest";
import { appCopy } from "../app/App";

describe("Ledger shell", () => {
  it("states that no financial records have been created", () => {
    expect(appCopy.detail).toContain("no financial data has been created");
    expect(appCopy.status).toBe("Offline local storage ready");
  });
});
