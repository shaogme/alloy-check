use std::process::Command;

#[test]
fn test_ron_output_for_non_compliant() {
    let bin_path = env!("CARGO_BIN_EXE_alloy-check");
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    
    let mut cmd = Command::new(bin_path);
    cmd.current_dir(manifest_dir);
    cmd.arg("--path").arg("tests/non_compliant");
    cmd.arg("--format").arg("ron");

    let output = cmd.output().expect("Failed to execute alloy-check");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Since we serialize to pretty RON format
    assert!(stdout.contains("diagnostics:"), "Output did not contain diagnostics:: {}", stdout);
    assert!(stdout.contains("file:"), "Output did not contain file:: {}", stdout);
    assert!(stdout.contains("severity: Error"), "Output did not contain severity: Error: {}", stdout);
    assert!(stdout.contains("code:"), "Output did not contain code:: {}", stdout);
    
    // The codebase is non_compliant, so it should exit with code 1
    assert_eq!(output.status.code(), Some(1), "Expected exit code 1, but got something else. Output: {}", stdout);
}
