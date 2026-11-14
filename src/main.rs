// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro` — the Pro command line. The commands live in the library
//! ([`kshana_pro::cli`]) so a server binary can run the same command line with a
//! registered extension ([`kshana_pro::extension`]).

fn main() -> std::process::ExitCode {
    kshana_pro::cli::main()
}
