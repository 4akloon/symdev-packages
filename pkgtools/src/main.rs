//! `pkgtools`: the build checks of the symdev-packages pipeline (the recipes' build.sh and
//! prebuilt.sh, symdev.yml) and the install test's static file server. Offline: no
//! bucket, no key. Exit 2 is a usage error, as with the Python tools these replace.

mod ar_archive;
mod casefold;
mod closure;
mod device_entry;
mod emulator_notices;
mod emulator_tree;
mod notices;
mod py_path;
mod py_text;
mod sdk_free;
mod serve;
mod target2;
mod tool_error;
mod tree_walk;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::casefold::IncludeOverlay;
use crate::closure::ClosureTool;
use crate::device_entry::DeviceEntry;
use crate::emulator_notices::EmulatorNotices;
use crate::emulator_tree::EmulatorTreeTool;
use crate::notices::NoticesTool;
use crate::sdk_free::SdkFreeTool;
use crate::serve::StaticServer;
use crate::target2::Target2Tool;

#[derive(Parser)]
#[command(name = "pkgtools", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Write THIRD-PARTY-NOTICES.txt for a static binary: every crate `cargo metadata
    /// --filter-platform <target>` resolves from <package> through normal dependencies (but
    /// the workspace's own), the code they bundle, and the Rust toolchain's runtime. Needs
    /// cargo and rustc.
    Notices {
        /// The checkout's Cargo.toml.
        #[arg(long, value_name = "Cargo.toml")]
        manifest_path: PathBuf,
        /// The binary's package, e.g. symdev-cli.
        #[arg(long)]
        package: String,
        /// e.g. x86_64-unknown-linux-musl.
        #[arg(long)]
        target: String,
        #[arg(long, value_name = "file")]
        output: PathBuf,
    },
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
    /// Fail unless build outputs hold nothing of the S60 SDK: no file (nor tar or .tar.gz
    /// member, nested ones too) with the bytes of an SDK file, none with a path through
    /// epoc32/. Exit 1 on a leak, 2 when the check cannot be made.
    SdkFree {
        #[arg(value_name = "sdk-dir")]
        sdk: PathBuf,
        /// A directory, tar or .tar.gz to search.
        #[arg(value_name = "path", required = true)]
        paths: Vec<PathBuf>,
    },
    /// The install test's fake bucket: serve <dir> on 127.0.0.1 at a free port, print the
    /// port, log each request to stderr; runs until killed.
    #[command(hide = true)]
    Serve {
        #[arg(value_name = "dir")]
        root: PathBuf,
    },
    /// Print one device's entry of an EKA2L1 devices.yml (the firmware recipe's
    /// device.yml). Exit 1 when it is missing or its firmcode differs.
    DeviceEntry {
        #[arg(value_name = "devices.yml")]
        devices: PathBuf,
        /// The device's key and firmware code, e.g. RM-469.
        #[arg(value_name = "firmcode")]
        firmcode: String,
    },
    /// Write share/doc/eka2l1/ into an extracted EKA2L1 AppImage: COPYING, every
    /// submodule's licence files, BUNDLED.tsv (with --packages) and SOURCE.txt.
    EmulatorNotices {
        #[arg(value_name = "eka2l1-src")]
        src: PathBuf,
        #[arg(value_name = "tree")]
        tree: PathBuf,
        #[arg(long, value_name = "id")]
        id: String,
        #[arg(long, value_name = "sha")]
        commit: String,
        #[arg(long, value_name = "packages.tsv")]
        packages: Option<PathBuf>,
        /// A file listing extra licence files, one path relative to <eka2l1-src> per line.
        #[arg(long, value_name = "list")]
        extra: Option<PathBuf>,
    },
    /// Check an extracted EKA2L1 AppImage for the layout symdev starts it by and for the
    /// glibc floor artifact.toml records. Exit 1 on any difference.
    EmulatorTree {
        #[arg(value_name = "tree")]
        tree: PathBuf,
        /// The recorded floor, e.g. 2.38.
        #[arg(long, value_name = "x.y")]
        glibc: String,
    },
    /// Rewrite every R_ARM_TARGET2 relocation of GCCE objects into R_ARM_ABS32, in place
    /// (symdev experiment 109): only the type byte of each relocation entry changes.
    #[command(name = "target2-abs32")]
    Target2Abs32 {
        #[arg(value_name = "object.o", required = true)]
        objects: Vec<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (mut out, mut err) = (io::stdout().lock(), io::stderr().lock());
    let code = match cli.command {
        Command::Notices {
            manifest_path,
            package,
            target,
            output,
        } => NoticesTool::run(
            &manifest_path,
            &package,
            &target,
            &output,
            &mut out,
            &mut err,
        ),
        Command::DeviceEntry { devices, firmcode } => match std::fs::read_to_string(&devices) {
            Ok(text) => match DeviceEntry::find(&text, &firmcode) {
                Ok(entry) => {
                    let _ = out.write_all(entry.text().as_bytes());
                    0
                }
                Err(e) => {
                    let _ = writeln!(err, "error: {}: {e}", devices.display());
                    1
                }
            },
            Err(e) => {
                let _ = writeln!(err, "error: {}: {e}", devices.display());
                1
            }
        },
        Command::EmulatorNotices {
            src,
            tree,
            id,
            commit,
            packages,
            extra,
        } => {
            let extra = extra
                .as_deref()
                .map_or(Ok(Vec::new()), EmulatorNotices::read_extra);
            let written = extra.and_then(|extra| {
                EmulatorNotices {
                    src,
                    tree,
                    id,
                    commit,
                    packages,
                    extra,
                }
                .write()
            });
            report(written.map(|(files, packages)| {
                format!(
                    "wrote share/doc/eka2l1: COPYING, {files} licence files, {packages} bundled \
                     packages, SOURCE.txt"
                )
            }))
        }
        Command::EmulatorTree { tree, glibc } => {
            EmulatorTreeTool::run(&tree, &glibc, &mut out, &mut err)
        }
        Command::RuntimeClosure { map, shipped } => {
            ClosureTool::run(&map, &shipped, &mut out, &mut err)
        }
        Command::Serve { root } => match StaticServer::bind(&root) {
            Ok(server) => {
                let _ = writeln!(out, "{}", server.port());
                let _ = out.flush();
                server.serve(err);
                0
            }
            Err(e) => report(Err(e)),
        },
        Command::Target2Abs32 { objects } => Target2Tool::run(&objects, &mut out, &mut err),
        Command::SdkFree { sdk, paths } => SdkFreeTool::run(&sdk, &paths, &mut out, &mut err),
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
