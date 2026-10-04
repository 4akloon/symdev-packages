//! `Dsc`: the files of a Debian source package and their SHA-256s, from its `.dsc`
//! (Debian Policy §5.4: the `Checksums-Sha256` field, one ` <sha256> <size> <name>` per line).

use crate::tool_error::{Result, ToolError};

#[derive(Debug)]
pub struct Dsc {
    files: Vec<(String, String)>,
}

impl Dsc {
    pub fn parse(text: &str) -> Result<Dsc> {
        let mut lines = text.lines().skip_while(|l| *l != "Checksums-Sha256:");
        if lines.next().is_none() {
            return Err(ToolError::new("the .dsc has no Checksums-Sha256 field"));
        }
        let mut files = Vec::new();
        for line in lines.take_while(|l| l.starts_with(' ')) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let [sha, _size, name] = fields[..] else {
                return Err(ToolError::new(format!(
                    "`{line}` is not ` <sha256> <size> <name>`"
                )));
            };
            let hex = sha.len() == 64 && sha.chars().all(|c| c.is_ascii_hexdigit());
            if !hex || name.contains('/') || name.starts_with('.') {
                return Err(ToolError::new(format!(
                    "`{line}`: not a sha256 and a plain file name"
                )));
            }
            files.push((sha.to_string(), name.to_string()));
        }
        if files.is_empty() {
            return Err(ToolError::new(
                "the .dsc's Checksums-Sha256 lists no file: a source package without files is not one",
            ));
        }
        Ok(Dsc { files })
    }

    /// `<sha256>  <name>` per file, as `sha256sum -c` reads it.
    pub fn sha256sums(&self) -> String {
        self.files
            .iter()
            .map(|(sha, name)| format!("{sha}  {name}\n"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Dsc;

    const SIGNED: &str = "-----BEGIN PGP SIGNED MESSAGE-----\nHash: SHA512\n\nFormat: 3.0 (quilt)\nSource: x264\nVersion: 2:0.164.3108+git31e19f9-1\nChecksums-Sha1:\n 1111111111111111111111111111111111111111 100 x264_0.164.orig.tar.gz\nChecksums-Sha256:\n aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 100 x264_0.164.orig.tar.gz\n bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 20 x264_0.164-1.debian.tar.xz\nFiles:\n 0123 100 x264_0.164.orig.tar.gz\n\n-----BEGIN PGP SIGNATURE-----\nxx\n-----END PGP SIGNATURE-----\n";

    #[test]
    fn lists_the_sha256_of_every_file_and_nothing_else() {
        let dsc = Dsc::parse(SIGNED).unwrap();
        assert_eq!(
            dsc.sha256sums(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  x264_0.164.orig.tar.gz\n\
             bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  x264_0.164-1.debian.tar.xz\n"
        );
    }

    #[test]
    fn a_dsc_without_sha256_checksums_is_refused() {
        let e = Dsc::parse("Source: x\nFiles:\n 0123 1 x.tar.gz\n").unwrap_err();
        assert!(e.to_string().contains("Checksums-Sha256"), "{e}");
    }

    #[test]
    fn an_empty_sha256_list_is_refused() {
        let e = Dsc::parse("Source: x\nChecksums-Sha256:\nFiles:\n 0123 1 x.tar.gz\n").unwrap_err();
        assert!(e.to_string().contains("lists no file"), "{e}");
    }

    #[test]
    fn a_file_name_with_a_path_is_refused() {
        let text = "Checksums-Sha256:\n aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 ../x.tar.gz\n";
        let e = Dsc::parse(text).unwrap_err();
        assert!(e.to_string().contains("../x.tar.gz"), "{e}");
    }
}
