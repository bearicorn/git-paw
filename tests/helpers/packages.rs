//! Accessors over the bundled skill package registry
//! ([`git_paw::skills::bundled_packages`]) for integration tests, so a test
//! reads or installs a bundled file by package and name rather than pinning
//! its source path under `assets/`.
//!
//! Included either as `helpers::packages` or directly with
//! `#[path = "helpers/packages.rs"] mod packages;`.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use git_paw::skills::{BundledPackage, bundled_package, bundled_packages};

/// The registered bundled package `name`; panics when it is not registered.
pub fn package(name: &str) -> &'static BundledPackage {
    bundled_package(name).unwrap_or_else(|| panic!("bundled package {name} is not registered"))
}

/// The full `SKILL.md` (frontmatter included) of the bundled package `name`.
pub fn skill_md(name: &str) -> &'static str {
    package(name).skill_md.content
}

/// The embedded content of the bundled helper script `file_name` (e.g.
/// `"sweep.sh"`, `"_paw_common.sh"`), whichever package ships it.
pub fn script(file_name: &str) -> &'static str {
    bundled_packages()
        .iter()
        .flat_map(|p| p.scripts)
        .find(|f| f.file_name() == file_name)
        .unwrap_or_else(|| panic!("no bundled package ships {file_name}"))
        .content
}

/// The repository source path the helper script `file_name` is embedded
/// from, for failure messages.
pub fn script_source(file_name: &str) -> &'static str {
    bundled_packages()
        .iter()
        .flat_map(|p| p.scripts)
        .find(|f| f.file_name() == file_name)
        .unwrap_or_else(|| panic!("no bundled package ships {file_name}"))
        .source
}

/// The embedded content of the file at `path` (package-relative, e.g.
/// `"assets/boot-block.md"`) of the bundled package `name`.
pub fn file(name: &str, path: &str) -> &'static str {
    package(name)
        .files()
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("package {name} has no file {path}"))
        .content
}

/// Installs every script of the bundled package `name` — its helper plus the
/// sibling `_paw_common.sh` preamble — into `dir` (created if missing), each
/// with mode `0o755`. Returns `dir`.
pub fn install_package(name: &str, dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("create package install dir");
    for file in package(name).scripts {
        let dst = dir.join(file.file_name());
        fs::write(&dst, file.content).unwrap_or_else(|e| panic!("write {}: {e}", dst.display()));
        set_executable(&dst);
    }
    dir.to_path_buf()
}

/// Installs the helper script `file_name` and the sibling `_paw_common.sh`
/// into `dir`, returning the installed helper's path.
pub fn install_helper(file_name: &str, dir: &Path) -> PathBuf {
    let owner = bundled_packages()
        .iter()
        .find(|p| p.scripts.iter().any(|f| f.file_name() == file_name))
        .unwrap_or_else(|| panic!("no bundled package ships {file_name}"));
    install_package(owner.name, dir).join(file_name)
}

/// Installs every bundled helper script (all packages, `_paw_common.sh` once)
/// into `dir`. Returns `dir`.
pub fn install_all_scripts(dir: &Path) -> PathBuf {
    for package in bundled_packages() {
        install_package(package.name, dir);
    }
    dir.to_path_buf()
}

#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod 0o755");
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) {}
