//! Prints a masked layout of a bank statement PDF so its structure can be shared safely.
//!
//!   cargo run --release --example pdf_layout -- "/path/to/statement.pdf"
//!
//! Everything runs on this Mac. Every digit is replaced with 9 and every letter with X or x
//! (except generic statement words such as "Date" or "Balance"), and the result is checked
//! before it is written. Read `pdf-layout-masked.txt` yourself before sharing it.

use std::{
    fs,
    io::{self, Write},
    process::{Command, ExitCode},
};

use ledger_lib::imports::pdf_layout::{extract_pages, masked_layout, PdfError};

fn read_password() -> String {
    eprint!("PDF password (typing is hidden): ");
    io::stderr().flush().ok();
    let _ = Command::new("stty").arg("-echo").status();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    let _ = Command::new("stty").arg("echo").status();
    eprintln!();
    line.trim_end_matches(['\n', '\r']).to_owned()
}

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run --release --example pdf_layout -- /path/to/statement.pdf");
        return ExitCode::FAILURE;
    };
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("could not read the file: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut encrypted = false;
    let pages = match extract_pages(&bytes, None) {
        Err(PdfError::NeedsPassword) => {
            encrypted = true;
            match extract_pages(&bytes, Some(&read_password())) {
                Ok(pages) => pages,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            }
        }
        Ok(pages) => pages,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    if pages.iter().all(|page| page.chars.is_empty()) {
        eprintln!("no text found. This PDF may be a scanned image, which Ledger cannot read yet.");
        return ExitCode::FAILURE;
    }
    match masked_layout(&pages, encrypted) {
        Ok(report) => {
            if let Err(error) = fs::write("pdf-layout-masked.txt", &report) {
                eprintln!("could not write the report: {error}");
                return ExitCode::FAILURE;
            }
            println!("{report}");
            eprintln!("Saved pdf-layout-masked.txt. Open it and check it before sharing.");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("masking check failed ({error}); nothing was written.");
            ExitCode::FAILURE
        }
    }
}
