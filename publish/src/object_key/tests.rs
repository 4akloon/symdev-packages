use super::ObjectKey;

fn refused(key: &str) -> String {
    match ObjectKey::parse(key) {
        Ok(_) => panic!("`{key}` was accepted"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_plain_path_is_a_key() {
    assert_eq!(
        ObjectKey::parse("install.sh").unwrap().as_str(),
        "install.sh"
    );
    assert_eq!(
        ObjectKey::parse("docs/v1.0/notes_~x-y.txt")
            .unwrap()
            .as_str(),
        "docs/v1.0/notes_~x-y.txt"
    );
}

#[test]
fn a_parent_segment_is_refused() {
    for key in ["..", "../install.sh", "a/../install.sh", "a/.."] {
        let e = refused(key);
        assert!(e.contains(&format!("`{key}`")) && e.contains("`..`"), "{e}");
    }
}

#[test]
fn a_leading_slash_is_refused() {
    let e = refused("/install.sh");
    assert!(
        e.contains("`/install.sh`") && e.contains("starts with `/`"),
        "{e}"
    );
}

#[test]
fn the_index_is_refused_because_only_a_package_publish_writes_it() {
    let e = refused("index.toml");
    assert!(
        e.contains("index.toml") && e.contains("publish public"),
        "{e}"
    );
}

#[test]
fn empty_keys_and_segments_are_refused() {
    for key in ["", "a//b", "a/", "./a", "a/./b"] {
        let e = refused(key);
        assert!(e.contains("empty") || e.contains("`.`"), "{key}: {e}");
    }
}

#[test]
fn characters_outside_the_unreserved_set_are_refused() {
    for key in [
        "a b",
        "a%20b",
        "a?b",
        "a#b",
        "a\\b",
        "https://x/a",
        "a\nb",
        "ä",
    ] {
        let e = refused(key);
        assert!(e.contains("A-Z a-z 0-9"), "{key:?}: {e}");
    }
}
