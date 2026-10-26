use std::process::Command;

#[test]
fn version_flag_prints_the_crate_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_httpmock"))
        .arg("--version")
        .output()
        .unwrap();

    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("httpmock {}\n", env!("CARGO_PKG_VERSION"))
    );
}
