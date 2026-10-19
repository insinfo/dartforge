//! LLVM installation selection must not depend on a developer's drive layout.
#[path = "../build_support/installation.rs"]
mod installation;

use installation::{compatible_version, select_prefix};
use std::path::PathBuf;

#[test]
fn explicit_configuration_wins_without_searching_path() {
    for (preferred, legacy, expected) in [
        (Some("/chosen path"), Some("/legacy"), "/chosen path"),
        (None, Some("/legacy"), "/legacy"),
        (Some(""), Some("/legacy"), "/legacy"),
    ] {
        let selected = select_prefix(preferred.map(Into::into), legacy.map(Into::into), |_, _| {
            panic!("an explicit installation must not fall through to PATH")
        })
        .unwrap();
        assert_eq!(selected, PathBuf::from(expected));
    }
}

#[test]
fn path_discovery_skips_incompatible_and_broken_tools() {
    let selected = select_prefix(None, None, |binary, arg| match (binary, arg) {
        ("llvm-config-22", "--version") => Ok("21.1.8\n".into()),
        ("llvm-config22", "--version") => Ok("22.1.8\n".into()),
        ("llvm-config22", "--prefix") => Err("broken prefix query".into()),
        ("llvm22-config", "--version") => Ok("22.1.8\n".into()),
        ("llvm22-config", "--prefix") => Ok(" \n".into()),
        ("llvm-config", "--version") => Ok("22.1.8\n".into()),
        ("llvm-config", "--prefix") => Ok("/LLVM path\n".into()),
        _ => Err("not installed".into()),
    })
    .unwrap();
    assert_eq!(selected, PathBuf::from("/LLVM path"));
}

#[test]
fn missing_tools_report_configuration_instead_of_assuming_an_installation() {
    let error = select_prefix(Some("".into()), Some("".into()), |_, _| {
        Err("not found".into())
    })
    .unwrap_err();
    assert!(
        error.contains("DARTFORGE_LLVM_DIR") && error.contains("PATH"),
        "{error}"
    );
}

#[test]
fn versioned_tool_name_has_the_same_priority_as_in_llvm_sys() {
    let selected = select_prefix(None, None, |binary, arg| {
        assert_eq!(binary, "llvm-config-22");
        Ok(if arg == "--version" {
            "22.1.8"
        } else {
            "/versioned"
        }
        .into())
    })
    .unwrap();
    assert_eq!(selected, PathBuf::from("/versioned"));
}

#[test]
fn only_the_supported_llvm_release_line_is_accepted() {
    for version in ["22.1.8\n", "22.1.0", "22.1.8git"] {
        assert!(compatible_version(version), "{version}");
    }
    for version in ["", "21.1.8", "23.1.0", "22.0.0", "22.2.0", "LLVM 22.1.8"] {
        assert!(!compatible_version(version), "{version}");
    }
}
