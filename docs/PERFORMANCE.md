# Performance results

Measured on an Apple M5 Mac running macOS 27.2, release build, on 2026-11-06. Data is synthetic (no real statements).

| Budget (spec section 11) | Target | Result |
| --- | --- | --- |
| 50,000-row CSV parse, preview and commit | ≤ 10 s | 0.78 s (preview 0.20 s, commit 0.59 s) |
| Filtered first-page transaction query, 50,000 rows | ≤ 300 ms | 7 ms |
| Overview summary, 50,000 rows, warm | ≤ 500 ms | 9 ms |
| Memory after 60 s idle, no statement loaded | ≤ 250 MB aggregate | about 180–200 MB (app 95 MB, web content 70 MB, GPU 27 MB, networking 6 MB) |
| Idle CPU | near 0% | 0% on helper processes at sampling. App process averaged 2.7% since launch, which includes startup. |
| Cold launch to usable first screen | ≤ 2.5 s | Not measured. It needs a UI-driven timer. |
| Re-importing an overlapping 50,000-row file with no reference numbers | ≤ 10 s | 0.24 s (all rows recognised, none added) |
| XLSX import at 50,000 rows | record separately | Not measured |

## Reproduce

```sh
cd src-tauri
cargo test --release --test performance -- --ignored --nocapture
```

The test fails if import exceeds 10 s, the page query exceeds 300 ms, or the summary exceeds 500 ms.

Memory was sampled with `ps` on the app process and the WebKit helpers it spawned, 65 seconds after `open Ledger.app`. Idle CPU was not sampled over a long window.

## Network and bundle inspection

- The production frontend bundle contains no `fetch`, XHR or WebSocket calls. The only URLs in it are XML namespaces and a React error-documentation string that is never requested.
- The Rust dependency tree has no HTTP client, updater, shell or opener plugin.
- The only granted capabilities are `dialog:allow-open` and `dialog:allow-save`.
- The content security policy allows `self` only.
