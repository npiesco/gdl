use assert_cmd::Command;

#[test]
fn nonexistent_repo_has_stable_error_prefix() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("gdl")?
        .args(["--repo", "/definitely/nonexistent", "status"])
        .output()?;

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.starts_with(
            "gdl: cannot open repo at /definitely/nonexistent: path does not exist: /definitely/nonexistent"
        ),
        "unexpected stderr: {stderr:?}"
    );

    Ok(())
}
