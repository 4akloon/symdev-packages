//! `publish`: packs a symdev package from its recipe and publishes it to a bucket
//! (design: symdev's `docs/superpowers/specs/2026-10-02-toolchain-manager-design.md` §6).

mod archive;
mod bucket;
mod file_upload;
mod include;
mod index_keys;
mod mode;
mod object_key;
mod publication;
mod recipe;
mod settings;
mod visibility;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use symdev_sdk::{Result, SdkError};

use crate::file_upload::FileUpload;
use crate::object_key::ObjectKey;
use crate::publication::Publication;
use crate::recipe::Recipe;
use crate::settings::Settings;
use crate::visibility::Visibility;

/// Packs a package from its recipe into `<sha256>.tar.gz` in the current directory, adds
/// it to the bucket's index.toml and uploads the archive, then the index.
///
/// Environment: PUBLISH_PRIVATE_URL and PUBLISH_PUBLIC_URL (the buckets' S3 endpoints),
/// PUBLISH_ACCESS_KEY_ID and PUBLISH_SECRET_ACCESS_KEY (the publisher key),
/// PUBLISH_SIGNING_KEY (the base64 Ed25519 seed every index is signed with). A --dry-run
/// needs none of them: it reads the index if the bucket URL is set, signs it if the
/// signing key is set, and uploads nothing.
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
    /// Sign the bucket's existing index.toml as it is and upload it (no-cache): how an
    /// index published unsigned gets its signature. Lists the archives it signs; an
    /// index whose signature does not verify, or that does not parse, is refused.
    SignIndex {
        /// The bucket whose index to sign.
        #[arg(long, value_name = "bucket")]
        bucket: BucketName,
        /// Read, check and print the signed index; upload nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Upload one file as it is to a fixed key (install.sh at the public bucket's root).
    /// The object is mutable: a later upload replaces it. The index is not touched.
    File {
        /// The local file.
        #[arg(value_name = "path")]
        path: PathBuf,
        /// The key in the bucket, relative to its root, e.g. 'install.sh'.
        #[arg(long, value_name = "key")]
        to: String,
        /// The bucket to upload to.
        #[arg(long, value_name = "bucket")]
        bucket: BucketName,
        /// The Content-Type header the object is served with.
        #[arg(long, value_name = "type")]
        content_type: String,
        /// The Cache-Control header the object is served with, e.g. 'no-cache'.
        #[arg(long, value_name = "value")]
        cache_control: String,
        /// Hash the file and say what would be uploaded; upload nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

/// `--bucket`: the bucket a plain file goes to.
#[derive(Clone, Copy, ValueEnum)]
enum BucketName {
    Public,
    Private,
}

impl From<BucketName> for Visibility {
    fn from(name: BucketName) -> Visibility {
        match name {
            BucketName::Public => Visibility::Public,
            BucketName::Private => Visibility::Private,
        }
    }
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
    match command {
        Command::Private {
            id,
            from,
            recipe,
            dry_run,
        } => publish(Visibility::Private, &id, &from, None, &recipe, dry_run),
        Command::Public {
            id,
            from,
            source_code,
            recipe,
            dry_run,
        } => publish(
            Visibility::Public,
            &id,
            &from,
            Some(&source_code),
            &recipe,
            dry_run,
        ),
        Command::SignIndex { bucket, dry_run } => {
            let settings = Settings::from_env();
            let mode = settings.mode(bucket.into(), dry_run)?;
            settings.index_keys(dry_run)?.resign(
                &mode,
                &mut io::stdout().lock(),
                &mut io::stderr().lock(),
            )
        }
        Command::File {
            path,
            to,
            bucket,
            content_type,
            cache_control,
            dry_run,
        } => {
            let key = ObjectKey::parse(&to)?;
            let mode = Settings::from_env().mode(bucket.into(), dry_run)?;
            let upload = FileUpload::new(&path, key, &content_type, &cache_control)?;
            upload.run(&mode, &mut io::stderr().lock())
        }
    }
}

/// `publish private|public`: one package of a recipe.
fn publish(
    visibility: Visibility,
    id: &str,
    from: &Path,
    source_code: Option<&Path>,
    recipe_path: &Path,
    dry_run: bool,
) -> Result<()> {
    let settings = Settings::from_env();
    let mode = settings.mode(visibility, dry_run)?;
    let keys = settings.index_keys(dry_run)?;
    let shown = recipe_path.display().to_string();
    let text = fs::read_to_string(recipe_path).map_err(|source| SdkError::Io {
        path: shown.clone(),
        source,
    })?;
    let recipe = Recipe::parse(&text, &shown, id)?;
    let publication = Publication::new(visibility, recipe, from, source_code)?;
    let out_dir = std::env::current_dir().map_err(|source| SdkError::Io {
        path: ".".into(),
        source,
    })?;
    publication.run(
        &mode,
        &keys,
        &out_dir,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}

#[cfg(test)]
mod tests;
