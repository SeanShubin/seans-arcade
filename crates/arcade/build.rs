use std::process::Command;

fn main() {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("failed to run git rev-parse HEAD");
    let hash = String::from_utf8(output.stdout).expect("invalid utf8 from git");
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", hash.trim());
}
