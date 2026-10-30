//! DOC-2: both READMEs embed `scripts/quickstart.sh` byte for byte.

fn read(path: &str) -> String {
    let full = format!("{}/{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&full).unwrap_or_else(|err| panic!("{path}: {err}"))
}

#[test]
fn readmes_embed_the_quickstart_script_exactly() {
    let script = read("scripts/quickstart.sh");
    assert!(script.ends_with('\n'), "the script must end with a newline");
    let block = format!("```sh\n{script}```\n");
    for readme in ["README.md", "README.ja.md"] {
        let text = read(readme);
        assert_eq!(
            text.matches(&block).count(),
            1,
            "{readme} must embed scripts/quickstart.sh exactly once, byte for byte"
        );
    }
}
