# Ledger user guide

Ledger keeps everything on this Mac. It never connects to a bank, a server or the internet.

## First run

1. Open **Import** and add an account. Ledger defaults to Canara Bank, the format it is built for.
2. Choose a CSV, XLSX or PDF statement. For XLSX, pick the worksheet. For a PDF see below.
3. For CSV and XLSX, confirm the column mapping. Check the preview: valid rows, date range, debit and credit totals, invalid rows and duplicate candidates.
4. Select **Confirm and import**. The import is all or nothing.

### Canara PDF statements

Ledger reads the PDF on your Mac. Nothing is uploaded. If the PDF is password-protected, Ledger asks for the password, uses it once and never saves it.

Ledger reads the Date, Particulars, Deposits, Withdrawals and Balance table and checks its work against the statement itself: every row's balance must follow from the row before it, and opening balance + deposits − withdrawals must equal the closing balance printed on the statement. The preview shows this as "Matches the statement's own balances". If it does not match, Ledger refuses to import that PDF and lists the rows it could not confirm.

Scanned or photographed statements have no text to read and cannot be imported. If your PDF layout looks different, Ledger says it does not recognise it instead of guessing.

Use one format per period. A PDF and a CSV of the same transactions word their descriptions differently, so importing both would add the transactions twice. Use Delete import or Delete by date range to undo that.

### Weekly updates

Upload each new statement even though it repeats earlier weeks. Ledger compares every row with what it already holds and adds only the new ones. The preview shows "11 new of 42 rows" with the amounts of the new rows, so you can check them against your statement before importing.

Rows are matched by reference number when the statement has one. Otherwise they are matched by date, direction, amount and description, counting duplicates. If you bought two identical coffees on one day and the file lists two, both are kept the first time and neither is added again later.

Matching is per account. If the bank changes how it words a description between exports, those rows look new and would be added twice. Delete that import and upload again, or delete the date range first.

### Deleting and re-uploading

Under the preview, **Import history** lists every upload with how many rows it added. **Delete import** removes the transactions that upload added. **Delete by date range** removes everything between two dates, for one account or all of them, and shows the count first.

Deleting also removes anything you reviewed or categorised on those transactions, and the confirmation tells you how many. If a later file repeated rows that an earlier upload added, deleting the earlier upload removes those rows too. Upload the later file again to bring them back.

## Reviewing transactions

**Transactions** lists everything, 50 per page. Filter by text, date range, account, direction, category or "needs review". Select a row to see the account, the source file and row, correct the merchant, choose a category and mark it reviewed. Categorising an unclassified transaction makes it count in totals.

## Overview

Shows spent, income and net cash movement for a period, each compared with the previous period. Pick a month, and widen the range to 3, 6 or 12 periods for categories, merchants and the trend. Spending excludes transfers and unknown transactions. "Possible recurring payments" are suggestions based on three or more months with similar amounts. Dismiss any that are wrong.

## Budgets and rules

**Budgets** set a monthly limit per category and show what is left and the per-day amount. **Settings** holds merchant rules, which apply only to future imports and never override your manual corrections.

## Settings

- **Preferences:** first day of the month (affects Overview only; budgets use calendar months) and where the database lives.
- **Accounts:** rename or archive. Archived accounts keep their transactions.
- **Categories:** add, rename or archive your own. Built-in ones are fixed.
- **Backups:** plain `.sqlite3`, or encrypted `.age` with a passphrase you must not lose. Restore validates the file before replacing data.
- **Export:** all transactions to CSV.
- **Clear personal data:** type `DELETE`. Create a backup first.

## Limits

Currency is INR only. There is no bank login or sync, and no PDF import. Google Pay and BHIM files are not supported yet, because that needs real sample exports.
