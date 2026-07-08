//! `volter new <name>` — scaffold a new volter project.
//!
//! Creates a minimal project tree:
//!
//! ```text
//! <name>/
//! ├── Cargo.toml
//! └── src/
//!     └── main.rs
//! ```

use std::fs;
use std::path::Path;

use anyhow::{bail, Context};

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

fn cargo_toml_template(project_name: &str, volter_version: &str) -> String {
    format!(
        r#"[package]
name = "{project_name}"
version = "0.1.0"
edition = "2021"
rust-version = "1.79"

[dependencies]
volter = "{volter_version}"
tokio = {{ version = "1", features = ["full"] }}
"#,
    )
}

fn main_rs_template(project_name: &str) -> String {
    format!(
        r#"//! {project_name} — created by `volter new`.

use tokio::net::TcpListener;
use volter::{{get, serve, Router}};

/// Root handler.
async fn root() -> &'static str {{
    "Hello, World!"
}}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {{
    let app = Router::new().route("/", get(root));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{{addr}}");

    serve(listener, app).await
}}
"#,
    )
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run `volter new` for the given project name.
///
/// The project is created in `parent_dir / name` (typically the current
/// working directory is the parent).
pub fn run_new_in(name: &str, parent_dir: &Path) -> anyhow::Result<()> {
    if name.is_empty() {
        bail!("project name cannot be empty");
    }

    if !is_valid_crate_name(name) {
        bail!(
            "`{name}` is not a valid Rust crate name. \
             Use only lowercase letters, digits, and hyphens."
        );
    }

    let project_dir = parent_dir.join(name);

    if project_dir.exists() {
        bail!("directory `{name}` already exists");
    }

    // Create directory structure.
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir).with_context(|| format!("failed to create `{}/src`", name))?;

    // Write Cargo.toml.
    let cargo_toml = cargo_toml_template(name, env!("CARGO_PKG_VERSION"));
    let cargo_path = project_dir.join("Cargo.toml");
    fs::write(&cargo_path, &cargo_toml)
        .with_context(|| format!("failed to write `{}`", cargo_path.display()))?;

    // Write src/main.rs.
    let main_rs = main_rs_template(name);
    let main_path = src_dir.join("main.rs");
    fs::write(&main_path, &main_rs)
        .with_context(|| format!("failed to write `{}`", main_path.display()))?;

    eprintln!("Created project `{name}`");
    Ok(())
}

/// Convenience wrapper that creates the project in the current directory.
pub fn run_new(name: &str) -> anyhow::Result<()> {
    run_new_in(
        name,
        &std::env::current_dir().context("failed to get current directory")?,
    )
}

/// Basic crate name validation: must start with a lowercase letter and contain
/// only lowercase letters, digits, and hyphens.
fn is_valid_crate_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let first = name.chars().next();
    if !first.is_some_and(|c| c.is_ascii_lowercase()) {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Create a unique temporary directory that is automatically cleaned up
    /// when the returned `TempDir` is dropped.
    struct TempDir {
        path: std::path::PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!("volter-test-{count}"));
            fs::create_dir_all(&path).expect("failed to create temp dir");
            Self { path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    // -- Template unit tests ------------------------------------------------

    #[test]
    fn cargo_toml_contains_project_name() {
        let result = cargo_toml_template("my-app", "0.1.0");
        assert!(result.contains(r#"name = "my-app""#));
        assert!(result.contains(r#"volter = "0.1.0""#));
        assert!(result.contains(r#"tokio = { version = "1", features = ["full"] }"#));
    }

    #[test]
    fn cargo_toml_contains_rust_version() {
        let result = cargo_toml_template("x", "0.1.0");
        assert!(result.contains("rust-version = \"1.79\""));
        assert!(result.contains("edition = \"2021\""));
    }

    #[test]
    fn main_rs_contains_imports_and_boilerplate() {
        let result = main_rs_template("test-app");
        assert!(result.contains("use tokio::net::TcpListener;"));
        assert!(result.contains("use volter::{get, serve, Router};"));
        assert!(result.contains("async fn root()"));
        assert!(result.contains("Router::new()"));
        assert!(result.contains("get(root)"));
        assert!(result.contains("serve(listener, app)"));
        assert!(result.contains("#[tokio::main]"));
    }

    // -- Integration tests --------------------------------------------------

    #[test]
    fn creates_project() {
        let temp = TempDir::new();
        let name = "test-proj";
        let project_path = temp.path().join(name);

        let result = run_new_in(name, temp.path());
        assert!(result.is_ok(), "run_new_in failed: {:?}", result.err());

        assert!(project_path.exists(), "project directory not created");
        assert!(
            project_path.join("Cargo.toml").exists(),
            "Cargo.toml not created"
        );
        assert!(
            project_path.join("src/main.rs").exists(),
            "src/main.rs not created"
        );
    }

    #[test]
    fn refuses_existing_directory() {
        let temp = TempDir::new();
        let name = "existing-dir";
        let project_path = temp.path().join(name);
        fs::create_dir(&project_path).unwrap();

        let result = run_new_in(name, temp.path());
        assert!(result.is_err(), "expected error for existing directory");
        let err = format!("{}", result.err().unwrap());
        assert!(
            err.contains("already exists"),
            "expected 'already exists' error, got: {err}"
        );
    }

    #[test]
    fn generated_cargo_toml_looks_correct() {
        let temp = TempDir::new();
        let name = "check-cargo";
        let project_path = temp.path().join(name);

        run_new_in(name, temp.path()).unwrap();

        let cargo = fs::read_to_string(project_path.join("Cargo.toml")).unwrap();
        assert!(cargo.contains(&format!("name = \"{name}\"")));
        assert!(cargo.contains("version = \"0.1.0\""));
        assert!(cargo.contains("edition = \"2021\""));
        assert!(cargo.contains("rust-version = \"1.79\""));
        assert!(cargo.contains("volter = "));
        assert!(cargo.contains("tokio = "));
    }

    #[test]
    fn generated_main_rs_looks_correct() {
        let temp = TempDir::new();
        let name = "check-main";
        let project_path = temp.path().join(name);

        run_new_in(name, temp.path()).unwrap();

        let main_rs = fs::read_to_string(project_path.join("src/main.rs")).unwrap();
        assert!(main_rs.contains("use tokio::net::TcpListener;"));
        assert!(main_rs.contains("use volter::{get, serve, Router};"));
        assert!(main_rs.contains("async fn root()"));
        assert!(main_rs.contains("Router::new()"));
        assert!(main_rs.contains("get(root)"));
        assert!(main_rs.contains("serve(listener, app)"));
    }

    #[test]
    fn rejects_invalid_names() {
        let temp = TempDir::new();
        let cases = &[
            ("UpperCase", "uppercase"),
            ("has spaces", "spaces"),
            ("", "empty"),
            ("123-starts-with-digit", "starts with digit"),
        ];
        for (name, _label) in cases {
            let result = run_new_in(name, temp.path());
            assert!(result.is_err(), "expected error for name `{name}`");
        }
    }
}
