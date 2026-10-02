//! `publish`: packs a symdev package from its recipe and publishes it to a bucket
//! (design: symdev's `docs/superpowers/specs/2026-10-02-toolchain-manager-design.md` §6).

mod archive;
mod bucket;
mod include;
mod mode;
mod publication;
mod recipe;
mod settings;
mod visibility;

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use symdev_sdk::{Result, SdkError};

use crate::publication::Publication;
use crate::recipe::Recipe;
use crate::settings::Settings;
use crate::visibility::Visibility;

/// Packs a package from its recipe into `<sha256>.tar.gz` in the current directory, adds
/// it to the bucket's index.toml and uploads the archive, then the index.
///
/// Environment: PUBLISH_PRIVATE_URL and PUBLISH_PUBLIC_URL (the buckets' S3 endpoints),
/// PUBLISH_ACCESS_KEY_ID and PUBLISH_SECRET_ACCESS_KEY (the publisher key). A --dry-run
/// needs none of them: it reads the index if the bucket URL is set and uploads nothing.
#[derive(Parser)]
#[command(name = "publish", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Publish a proprietary package (the SDK) to the private bucket.
    Private {
        /// The package id, e.g. 'sdk;s60-3rd-fp2;1.1'.
        #[arg(value_name = "id")]
        id: String,
        /// The tree the recipe's include list is taken from.
        #[arg(long, value_name = "dir")]
        from: PathBuf,
        /// The package's recipe (id, licence, host, include list, pinned sha256).
        #[arg(long, value_name = "recipe.toml")]
        recipe: PathBuf,
        /// Pack, check and print the new index; upload nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Publish a built package and its corresponding source to the public bucket.
    Public {
        /// The package id, e.g. 'gcce;12.1.0'.
        #[arg(value_name = "id")]
        id: String,
        /// The installed prefix to pack.
        #[arg(long, value_name = "prefix")]
        from: PathBuf,
        /// The corresponding source (sources, patches, build script) as one archive.
        #[arg(long, value_name = "tar.gz")]
        source_code: PathBuf,
        /// The package's recipe (id, licence, host, include list, pinned sha256).
        #[arg(long, value_name = "recipe.toml")]
        recipe: PathBuf,
        /// Pack, check and print the new index; upload nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<()> {
    let (visibility, id, from, source_code, recipe_path, dry_run) = match command {
        Command::Private {
            id,
            from,
            recipe,
            dry_run,
        } => (Visibility::Private, id, from, None, recipe, dry_run),
        Command::Public {
            id,
            from,
            source_code,
            recipe,
            dry_run,
        } => (
            Visibility::Public,
            id,
            from,
            Some(source_code),
            recipe,
            dry_run,
        ),
    };
    let mode = Settings::from_env().mode(visibility, dry_run)?;
    let shown = recipe_path.display().to_string();
    let text = fs::read_to_string(&recipe_path).map_err(|source| SdkError::Io {
        path: shown.clone(),
        source,
    })?;
    let recipe = Recipe::parse(&text, &shown)?;
    let publication = Publication::new(visibility, &id, recipe, &from, source_code.as_deref())?;
    let out_dir = std::env::current_dir().map_err(|source| SdkError::Io {
        path: ".".into(),
        source,
    })?;
    publication.run(
        &mode,
        &out_dir,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}

#[cfg(test)]
mod tests;
