//! `pkgtools`: the build checks of the symdev-packages pipeline (the recipes' build.sh and
//! prebuilt.sh, symdev.yml) and the install test's static file server. Offline: no
//! bucket, no key. Exit 2 is a usage error, as with the Python tools these replace.

mod closure;
mod py_text;
mod tool_error;

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (mut out, mut err) = (io::stdout().lock(), io::stderr().lock());
    let code = match cli.command {
        Command::RuntimeClosure { map, shipped } => {
            ClosureTool::run(&map, &shipped, &mut out, &mut err)
        }
    };
    ExitCode::from(code)
}
