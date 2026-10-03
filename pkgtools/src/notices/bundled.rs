//! `Bundled`: code inside a crate that has a licence of its own.

use super::component::Component;
use super::{Crate, Entry};
use crate::tool_error::{Result, ToolError};

/// Every crate with `links` (native code) and every licence file below a crate's top must
/// be named here: as a component the binary links, or under `not_linked` with the reason
/// (checked against the crate's build.rs).
pub struct Bundled {
    krate: &'static str,
    components: &'static [Component],
    not_linked: &'static [(&'static str, &'static str)],
}

const WHERE: &str = "BUNDLED in pkgtools/src/notices/bundled.rs";

const BUNDLED: &[Bundled] = &[
    Bundled {
        krate: "libz-sys",
        components: &[Component {
            title: "zlib {version}",
            license: "Zlib",
            files: &["src/zlib/LICENSE"],
            version: Some(("src/zlib/zlib.h", "#define ZLIB_VERSION \"")),
            note: "The C library in src/zlib, compiled by libz-sys' build script and linked \
                   statically (build.sh sets LIBZ_SYS_STATIC=1, so no system libz is used).",
        }],
        not_linked: &[
            (
                "src/zlib-ng/LICENSE.md",
                "zlib-ng is built only with libz-sys' zlib-ng features",
            ),
            (
                "src/zlib/contrib/dotzlib/LICENSE_1_0.txt",
                "zlib's contrib/ is not compiled",
            ),
            (
                "src/zlib/contrib/minizip/LICENSE.Info-Zip",
                "zlib's contrib/ is not compiled",
            ),
        ],
    },
    Bundled {
        krate: "ring",
        components: &[
            Component {
                title: "BoringSSL-derived C and assembly",
                license: "Apache-2.0 AND ISC",
                files: &["LICENSE", "LICENSE-BoringSSL", "LICENSE-other-bits"],
                version: None,
                note: "crypto/, include/ and the pregenerated assembly, compiled by ring's \
                       build script; each file's header names its licence.",
            },
            Component {
                title: "fiat-crypto",
                license: "Apache-2.0",
                files: &["third_party/fiat/LICENSE"],
                version: None,
                note: "third_party/fiat: C headers and x86_64 assembly ring compiles.",
            },
            Component {
                title: "once_cell (ring's polyfill)",
                license: "MIT OR Apache-2.0",
                files: &[
                    "src/polyfill/once_cell/LICENSE-APACHE",
                    "src/polyfill/once_cell/LICENSE-MIT",
                ],
                version: None,
                note: "Rust code from once_cell in src/polyfill/once_cell, compiled into ring.",
            },
        ],
        not_linked: &[],
    },
];

impl Bundled {
    /// The entries of the code `krate` bundles; an error for a crate with native code or
    /// a nested licence file the table does not account for.
    pub fn entries(krate: &Crate) -> Result<Vec<Entry>> {
        let nested = krate.nested_notice_files()?;
        let Some(table) = BUNDLED.iter().find(|t| t.krate == krate.name) else {
            if krate.links.is_none() && nested.is_empty() {
                return Ok(Vec::new());
            }
            let why = match &krate.links {
                Some(links) => format!("links `{links}`"),
                None => format!("has {}", nested.join(", ")),
            };
            return Err(ToolError::new(format!(
                "{} {} {why}: find the licence of the code it bundles and add {} to {WHERE}",
                krate.name, krate.version, krate.name
            )));
        };
        let named = |rel: &str| {
            table.components.iter().any(|c| c.files.contains(&rel))
                || table.not_linked.iter().any(|(file, _)| *file == rel)
        };
        if let Some(rel) = nested.iter().find(|rel| !named(rel)) {
            return Err(ToolError::new(format!(
                "{} {} has {rel}, which {WHERE} does not name; decide whether that code is linked",
                krate.name, krate.version
            )));
        }
        table
            .components
            .iter()
            .map(|c| Self::entry(krate, c))
            .collect()
    }

    fn entry(krate: &Crate, component: &Component) -> Result<Entry> {
        let mut title = component.title.to_string();
        if let Some((rel, before)) = component.version {
            let text = krate.text(rel)?;
            let version = Self::version_in(&text, before).ok_or_else(|| {
                ToolError::new(format!(
                    "no version in {} (looked for {before}<version>\")",
                    krate.shown(rel)
                ))
            })?;
            title = title.replace("{version}", version);
        }
        let files = component
            .files
            .iter()
            .map(|rel| Ok((rel.to_string(), krate.text(rel)?)))
            .collect::<Result<_>>()?;
        Ok(Entry {
            title: format!("{title} (in {} {})", krate.name, krate.version),
            license: component.license.into(),
            files,
            source: String::new(),
            note: component.note.into(),
        })
    }

    /// `re.search(before + '([^"]+)"', text)`: the first non-empty text after `before` that
    /// a `"` ends.
    fn version_in<'t>(text: &'t str, before: &str) -> Option<&'t str> {
        text.match_indices(before).find_map(|(at, _)| {
            let rest = &text[at + before.len()..];
            rest.find('"')
                .filter(|&end| end > 0)
                .map(|end| &rest[..end])
        })
    }
}
