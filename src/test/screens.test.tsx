// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("../app/commands", () => ({
  listAccounts: vi.fn(),
  createAccount: vi.fn(),
  listCategories: vi.fn(),
  listTransactions: vi.fn(),
  editTransaction: vi.fn(),
  getOverviewSummary: vi.fn(),
  recurringSuggestions: vi.fn(),
  dismissRecurring: vi.fn(),
  getSettings: vi.fn(),
  setFirstDayOfMonth: vi.fn(),
  renameAccount: vi.fn(),
  archiveAccount: vi.fn(),
  createCategory: vi.fn(),
  renameCategory: vi.fn(),
  archiveCategory: vi.fn(),
  backupDatabaseEncrypted: vi.fn(),
  restoreDatabaseEncrypted: vi.fn()
}));

import * as commands from "../app/commands";
import { ImportScreen } from "../features/imports/ImportScreen";
import { OverviewScreen } from "../features/overview/OverviewScreen";
import { ManagementPanels } from "../features/settings/ManagementPanels";
import { TransactionsScreen } from "../features/transactions/TransactionsScreen";

const mocked = vi.mocked(commands);

const transaction = {
  id: "t1",
  transactionDate: "2026-03-03",
  direction: "debit",
  amountPaise: 124850,
  rawDescription: "UPI/SWIGGY/4482",
  merchantOverride: null,
  categoryId: null,
  transactionKind: "unknown",
  isReviewed: false,
  accountName: "Daily account",
  categoryName: null,
  sourceFilename: "march.csv",
  importedAt: "2026-04-01T10:00:00Z",
  sourceRowNumber: 12
};

const summary = {
  periodLabel: "2026-03",
  rangeMonths: 1,
  firstDayOfMonth: 1,
  periodStart: "2026-03-01",
  periodEnd: "2026-03-15",
  incomePaise: 1000000,
  expensePaise: 250000,
  netCashPaise: 750000,
  previousIncomePaise: 900000,
  previousExpensePaise: 300000,
  unreviewedCount: 3,
  lastImportedAt: "2026-03-12",
  coverageStart: "2026-01-01",
  coverageEnd: "2026-03-10",
  categories: [["Groceries", 250000]] as [string, number][],
  trend: [["2026-03", 250000]] as [string, number][],
  merchants: [["Swiggy", 250000]] as [string, number][],
  budgetAlerts: [] as [string, number, number][]
};

beforeEach(() => {
  vi.clearAllMocks();
  mocked.listAccounts.mockResolvedValue([{ id: "a1", name: "Daily account", institution: "Canara Bank", accountLabel: null }]);
  mocked.listCategories.mockResolvedValue([
    { id: "c1", name: "Groceries", kind: "expense", isSystem: true },
    { id: "c2", name: "Pet care", kind: "expense", isSystem: false }
  ]);
  mocked.listTransactions.mockResolvedValue({ items: [transaction], total: 1 });
  mocked.editTransaction.mockResolvedValue(undefined);
  mocked.getOverviewSummary.mockResolvedValue(summary);
  mocked.recurringSuggestions.mockResolvedValue([
    { merchantKey: "netflix", merchant: "Netflix", typicalAmountPaise: 64900, monthsSeen: 4, lastSeen: "2026-03-01" }
  ]);
  mocked.dismissRecurring.mockResolvedValue(undefined);
  mocked.getSettings.mockResolvedValue({ firstDayOfMonth: 1, currency: "INR", databasePath: "/data/ledger.sqlite3" });
  mocked.setFirstDayOfMonth.mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("Transactions screen", () => {
  it("passes filters to the backend and resets to the first page", async () => {
    const user = userEvent.setup();
    render(<TransactionsScreen onError={vi.fn()} />);
    await screen.findByText("UPI/SWIGGY/4482");
    await user.selectOptions(screen.getByLabelText("Direction"), "debit");
    await user.selectOptions(screen.getByLabelText("Category"), "c1");
    await waitFor(() =>
      expect(mocked.listTransactions).toHaveBeenLastCalledWith(
        expect.objectContaining({ direction: "debit", categoryId: "c1", offset: 0, limit: 50, reviewOnly: false })
      )
    );
  });

  it("shows source details and saves a chosen category and review", async () => {
    const user = userEvent.setup();
    render(<TransactionsScreen onError={vi.fn()} />);
    await user.click(await screen.findByText("UPI/SWIGGY/4482"));
    expect(screen.getByText(/march\.csv, row 12/)).toBeInTheDocument();
    const panel = screen.getByRole("heading", { name: "Review transaction" }).closest(".panel") as HTMLElement;
    await user.selectOptions(within(panel).getByLabelText("Category"), "c1");
    await user.click(within(panel).getByLabelText("Reviewed"));
    await user.click(within(panel).getByRole("button", { name: "Save review" }));
    await waitFor(() =>
      expect(mocked.editTransaction).toHaveBeenCalledWith("t1", { merchantOverride: "", categoryId: "c1", isReviewed: true })
    );
  });

  it("explains an empty database and an empty filter result differently", async () => {
    const user = userEvent.setup();
    mocked.listTransactions.mockResolvedValue({ items: [], total: 0 });
    render(<TransactionsScreen onError={vi.fn()} />);
    expect(await screen.findByText(/Import a statement to see them here/)).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("Direction"), "credit");
    expect(await screen.findByText("No transactions match these filters.")).toBeInTheDocument();
  });

  it("reports backend errors instead of failing silently", async () => {
    const onError = vi.fn();
    mocked.listTransactions.mockRejectedValue("database unavailable");
    render(<TransactionsScreen onError={onError} />);
    await waitFor(() => expect(onError).toHaveBeenCalledWith("database unavailable"));
  });
});

describe("Overview screen", () => {
  it("formats rupees, compares to the previous month and exposes an accessible chart summary", async () => {
    render(<OverviewScreen onError={vi.fn()} />);
    expect(await screen.findByText("2026-03")).toBeInTheDocument();
    const spent = screen.getByText("Spent").closest(".metric") as HTMLElement;
    expect(within(spent).getByText("₹2,500.00")).toBeInTheDocument();
    expect(screen.getByText("−₹500.00 vs previous month")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: /2026-03: ₹2,500\.00/ })).toBeInTheDocument();
  });

  it("requests a wider range and a chosen month", async () => {
    const user = userEvent.setup();
    render(<OverviewScreen onError={vi.fn()} />);
    await screen.findByText("2026-03");
    await user.selectOptions(screen.getByLabelText("Range"), "6");
    await waitFor(() =>
      expect(mocked.getOverviewSummary).toHaveBeenLastCalledWith(expect.any(String), { month: undefined, rangeMonths: 6 })
    );
  });

  it("lets the user dismiss a recurring suggestion", async () => {
    const user = userEvent.setup();
    render(<OverviewScreen onError={vi.fn()} />);
    await user.click(await screen.findByRole("button", { name: "Not recurring" }));
    expect(mocked.dismissRecurring).toHaveBeenCalledWith("netflix");
  });
});

describe("Settings management", () => {
  it("saves the first day of the month", async () => {
    const user = userEvent.setup();
    render(<ManagementPanels onError={vi.fn()} />);
    const input = await screen.findByLabelText(/First day of month/);
    await user.clear(input);
    await user.type(input, "5");
    await user.click(screen.getAllByRole("button", { name: "Save" })[0]);
    expect(mocked.setFirstDayOfMonth).toHaveBeenCalledWith(5);
    expect(await screen.findByText(/Saved\. Overview months/)).toBeInTheDocument();
  });

  it("requires a long enough passphrase before an encrypted backup", async () => {
    const user = userEvent.setup();
    render(<ManagementPanels onError={vi.fn()} />);
    const create = await screen.findByRole("button", { name: "Create encrypted backup" });
    expect(create).toBeDisabled();
    await user.type(screen.getByLabelText(/Passphrase/), "long enough pass");
    expect(create).toBeEnabled();
  });

  it("does not archive an account unless the user confirms", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<ManagementPanels onError={vi.fn()} />);
    const row = (await screen.findByText("Daily account")).closest("li") as HTMLElement;
    await user.click(within(row).getByRole("button", { name: "Archive" }));
    expect(confirm).toHaveBeenCalled();
    expect(mocked.archiveAccount).not.toHaveBeenCalled();
    confirm.mockReturnValue(true);
    await user.click(within(row).getByRole("button", { name: "Archive" }));
    expect(mocked.archiveAccount).toHaveBeenCalledWith("a1");
  });
});

describe("Import screen", () => {
  it("shows a useful empty state without accounts or fake data", async () => {
    mocked.listAccounts.mockResolvedValue([]);
    render(<ImportScreen onError={vi.fn()} />);
    expect(await screen.findByText("Create a local account first")).toBeInTheDocument();
    expect(screen.getByText("No bank credentials are requested or stored.")).toBeInTheDocument();
  });
});
