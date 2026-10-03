//! `pkgtools`: the build checks of the symdev-packages pipeline (the recipes' build.sh and
//! prebuilt.sh, symdev.yml) and the install test's static file server. Offline: no
//! bucket, no key. Exit 2 is a usage error, as with the Python tools these replace.

mod casefold;
mod closure;
mod py_path;
mod py_text;
mod tool_error;
mod tree_walk;

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::casefold::IncludeOverlay;
use crate::closure::ClosureTool;

#[derive(Parser)]
#[command(name = "pkgtools", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fail unless the GCC runtime members a GNU ld link map took are exactly the shipped
    /// ones (prebuilt.sh, step 4); prints each member taken and why.
    RuntimeClosure {
        /// The -Map of the closure link.
        #[arg(value_name = "ld.map")]
        map: PathBuf,
        /// A shipped member, e.g. 'libgcc.a(pr-support.o)'.
        #[arg(value_name = "archive(member)", required = true)]
        shipped: Vec<String>,
    },
    /// Build (once) a case-insensitive overlay of the SDK's epoc32/include: a symlink for
    /// every include name the tree has only in another case. Prints the overlay's path.
    SdkCasefold {
        #[arg(value_name = "epoc32/include")]
        include: PathBuf,
        #[arg(value_name = "out-dir")]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (mut out, mut err) = (io::stdout().lock(), io::stderr().lock());
    let code = match cli.command {
        Command::RuntimeClosure { map, shipped } => {
            ClosureTool::run(&map, &shipped, &mut out, &mut err)
        }
        Command::SdkCasefold { include, out: dir } => {
            report(IncludeOverlay::ensure(&include, &dir).map(|o| o.display().to_string()))
        }
    };
    ExitCode::from(code)
}

/// Prints what a tool made, or `error: …` and exit 1.
fn report(made: tool_error::Result<String>) -> u8 {
    match made {
        Ok(line) => {
            println!("{line}");
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}
