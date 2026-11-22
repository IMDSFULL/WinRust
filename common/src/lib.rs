//! Code shared by the WinRust programs: the language switch (`t!`, `tf!`, `en!`, `pt!`), the NTFS MFT reader, Win+D.
//! Use it with `#[macro_use] extern crate winrust_common;` at the crate root.
//!
//! English by default; Russian when %ProgramData%\WinRust\lang.txt contains "ru", Brazilian Portuguese when it contains "pt"
//! (the language button of the installer and the bar's right-click menu write it).
//! Programs read the file once at start; the one that flips the switch updates its own texts at once.
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU8, Ordering::Relaxed},
};

pub mod desktop;
pub mod journal;
pub mod mem;
pub mod mft;
pub mod proc;
pub mod reg;
pub mod settings;
pub mod style;

/// "Show in folder": opens `path`'s folder with the file selected in the installed rust-explorer, else in Windows Explorer.
pub fn reveal(path: &str) {
    use std::{os::windows::process::CommandExt, process::Command};
    let rx = std::env::var_os("ProgramFiles").map(|pf| PathBuf::from(pf).join(r"rust-explorer\rust-explorer.exe")).filter(|p| p.is_file());
    let _ = match rx {
        Some(rx) => Command::new(rx).raw_arg(format!("/select,\"{path}\"")).spawn(),
        None => Command::new("explorer.exe").raw_arg(format!("/select,\"{path}\"")).spawn(),
    };
}

/// The languages, as stored in `LANG` (0 = not read yet).
pub const EN: u8 = 1;
pub const RU: u8 = 2;
pub const PT: u8 = 3;

static LANG: AtomicU8 = AtomicU8::new(0);

fn file() -> Option<PathBuf> {
    std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join("WinRust").join("lang.txt"))
}

/// `lang.txt` content -> language ("ru", "pt" / "pt-br" / "pt_BR"; anything else is English).
fn parse(s: &str) -> u8 {
    match s.trim().to_ascii_lowercase().as_str() {
        "ru" => RU,
        "pt" | "pt-br" | "pt_br" => PT,
        _ => EN,
    }
}

/// The current language (`EN`, `RU` or `PT`), read from the file on first use.
pub fn lang() -> u8 {
    match LANG.load(Relaxed) {
        0 => {
            let l = file().and_then(|f| std::fs::read_to_string(f).ok()).map_or(EN, |s| parse(&s));
            LANG.store(l, Relaxed);
            l
        }
        l => l,
    }
}

/// True unless the language is Russian: English and Portuguese share the English code paths, and Portuguese text is picked
/// on top of that by `t!` / `tf!` when they have a third (Portuguese) text. In code that has unit tests use `en!()` instead:
/// it is always Russian under `cargo test`.
pub fn english() -> bool {
    lang() != RU
}

/// True for Brazilian Portuguese.
pub fn portuguese() -> bool {
    lang() == PT
}

/// Switches this process and writes the choice for the other programs (they pick it up when they start).
pub fn set_lang(l: u8) {
    let l = if matches!(l, RU | PT) { l } else { EN };
    LANG.store(l, Relaxed);
    if let Some(f) = file() {
        let _ = std::fs::create_dir_all(f.parent().unwrap());
        let _ = std::fs::write(f, match l { RU => "ru", PT => "pt", _ => "en" });
    }
}

/// Kept for callers that only know English / Russian.
pub fn set_english(en: bool) {
    set_lang(if en { EN } else { RU });
}

/// The language after `l` in the switch button's order: English, Portuguese, Russian, English...
pub fn next_lang(l: u8) -> u8 {
    match l {
        EN => PT,
        PT => RU,
        _ => EN,
    }
}

/// `en!()`: true unless Russian (English, or Portuguese falling back to English). `cfg!(test)` is checked in the calling crate,
/// so its unit tests keep their Russian expectations.
#[macro_export]
macro_rules! en {
    () => {
        !cfg!(test) && $crate::english()
    };
}

/// `pt!()`: true for Brazilian Portuguese (never under `cargo test`).
#[macro_export]
macro_rules! pt {
    () => {
        !cfg!(test) && $crate::portuguese()
    };
}

/// `t!("русский", "english")` or `t!("русский", "english", "português")`: a text in the current language.
/// Without the third text Portuguese shows the English one.
#[macro_export]
macro_rules! t {
    ($ru:expr, $en:expr, $pt:expr $(,)?) => {
        if $crate::pt!() { $pt } else if $crate::en!() { $en } else { $ru }
    };
    ($ru:expr, $en:expr) => {
        if $crate::en!() { $en } else { $ru }
    };
}

/// `tf!("русский {}", "english {}", args...)` or `tf!("русский {}", "english {}", pt: "português {}", args...)`:
/// format! in the current language (all templates are checked at compile time). Without `pt:` Portuguese shows the English one.
#[macro_export]
macro_rules! tf {
    ($ru:literal, $en:literal, pt: $pt:literal $(, $a:expr)* $(,)?) => {
        if $crate::pt!() { format!($pt $(, $a)*) } else if $crate::en!() { format!($en $(, $a)*) } else { format!($ru $(, $a)*) }
    };
    ($ru:literal, $en:literal $(, $a:expr)* $(,)?) => {
        if $crate::en!() { format!($en $(, $a)*) } else { format!($ru $(, $a)*) }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn tests_run_in_russian() {
        assert_eq!(t!("да", "yes"), "да");
        assert_eq!(tf!("{} шт", "{} pcs", 3), "3 шт");
        assert_eq!(t!("да", "yes", "sim"), "да");
        assert_eq!(tf!("{} шт", "{} pcs", pt: "{} un.", 3), "3 шт");
    }

    #[test]
    fn lang_file_values() {
        use super::{EN, PT, RU, next_lang, parse};
        assert_eq!((parse("ru"), parse(" PT-BR\r\n"), parse("pt_br"), parse("en"), parse("")), (RU, PT, PT, EN, EN));
        assert_eq!((next_lang(EN), next_lang(PT), next_lang(RU)), (PT, RU, EN)); // every language is reachable
    }
}
