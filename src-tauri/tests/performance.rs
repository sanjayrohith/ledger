//! Performance budgets from PROJECT_SPEC section 11. Run explicitly:
//! `cargo test --release --test performance -- --ignored --nocapture`
use std::{fmt::Write as _, fs, time::Instant};

use ledger_lib::db::{
    repositories::{
        AccountDraft, AccountRepository, OverviewRepository, TransactionQuery,
        TransactionRepository,
    },
    Database,
};
use ledger_lib::imports::csv::CsvImportService;

const ROWS: usize = 50_000;

fn synthetic_csv() -> String {
    let mut text = String::from("Transaction Date,Narration,Withdrawal Amt.,Deposit Amt.,UTR\n");
    for index in 0..ROWS {
        let day = 1 + index % 28;
        let month = 1 + (index / 28) % 12;
        let year = 2023 + (index / 336) % 3;
        if index % 9 == 0 {
            writeln!(
                text,
                "{year}-{month:02}-{day:02},SYNTHETIC SALARY {index},,{}.00,SYN-{index:06}",
                1000 + index % 500
            )
            .unwrap();
        } else {
            writeln!(
                text,
                "{year}-{month:02}-{day:02},SYNTHETIC MERCHANT {},{}.{:02},,SYN-{index:06}",
                index % 200,
                10 + index % 900,
                index % 100
            )
            .unwrap();
        }
    }
    text
}

#[test]
#[ignore = "performance measurement, run on demand"]
fn fifty_thousand_rows_meet_the_spec_budgets() {
    let database = Database::open_in_memory().unwrap();
    let account = AccountRepository::new(&database)
        .create(AccountDraft {
            name: "Perf".into(),
            institution: None,
            account_label: None,
        })
        .unwrap();
    let path = std::env::temp_dir().join(format!("ledger-perf-{}.csv", std::process::id()));
    fs::write(&path, synthetic_csv()).unwrap();
    let path_text = path.to_str().unwrap();
    let service = CsvImportService::new(&database);

    let started = Instant::now();
    let preview = service.preview(&account.id, path_text, None).unwrap();
    let preview_time = started.elapsed();
    let commit_started = Instant::now();
    let committed = service
        .commit(&account.id, path_text, preview.mapping.clone())
        .unwrap();
    let total_import = started.elapsed();
    println!(
        "rows seen {} imported {}",
        committed.rows_seen, committed.rows_imported
    );
    println!("PERF preview {:?}", preview_time);
    println!("PERF commit  {:?}", commit_started.elapsed());
    println!(
        "PERF import total (preview+commit) {:?}  budget 10s",
        total_import
    );

    let page_started = Instant::now();
    let page = TransactionRepository::new(&database)
        .page(TransactionQuery {
            search: Some("MERCHANT 7".into()),
            review_only: true,
            limit: 50,
            ..Default::default()
        })
        .unwrap();
    let page_time = page_started.elapsed();
    println!(
        "PERF filtered first page ({} matches) {:?}  budget 300ms",
        page.total, page_time
    );

    let overview = OverviewRepository::new(&database);
    overview.summary("2025-06-15").unwrap();
    let summary_started = Instant::now();
    overview.summary("2025-06-15").unwrap();
    let summary_time = summary_started.elapsed();
    println!(
        "PERF overview summary (warm) {:?}  budget 500ms",
        summary_time
    );

    fs::remove_file(path).unwrap();
    assert!(total_import.as_secs_f64() <= 10.0, "import over budget");
    assert!(page_time.as_millis() <= 300, "page query over budget");
    assert!(summary_time.as_millis() <= 500, "overview over budget");
}
