//! Native package resolution and cache population; no Dart/Pub subprocess delegation.
mod constraints;
mod hosted;
mod manifest;
mod output;
mod resolver;
mod sdk;
#[cfg(test)]
mod tests;
use std::{ffi::OsString, path::PathBuf};

/// Resolves the supported path and hosted package sources and writes Pub artifacts.
pub(crate) fn run(args: &[OsString]) -> Result<(), String> {
    let usage = "usage: dartforge pub get [--offline] [--directory <dir>] [--sdk <lib>] [--sdk-version <version>]";
    let mut directory = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut offline = false;
    let mut sdk_version = None;
    let mut sdk_lib = None;
    let mut index = 0;
    while index < args.len() {
        match args[index]
            .to_str()
            .ok_or("pub get arguments must be UTF-8")?
        {
            "--offline" => offline = true,
            "--directory" | "--sdk-version" | "--sdk" => {
                let option = args[index].to_str().unwrap();
                index += 1;
                let value = args.get(index).ok_or(usage)?;
                if option == "--directory" {
                    directory = PathBuf::from(value);
                } else if option == "--sdk" {
                    sdk_lib = Some(PathBuf::from(value));
                } else {
                    sdk_version = Some(
                        value
                            .to_str()
                            .ok_or("SDK version must be UTF-8")?
                            .to_owned(),
                    );
                }
            }
            "--help" => {
                println!("{usage}");
                return Ok(());
            }
            _ => return Err(usage.into()),
        }
        index += 1;
    }
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    let sdk_lib = sdk_lib.or_else(dartforge_elements::sdk::SdkLayout::discover);
    let sdk_root = sdk_lib
        .map(|lib| {
            let lib = lib
                .canonicalize()
                .map_err(|error| format!("{}: {error}", lib.display()))?;
            if !lib.join("libraries.json").is_file() {
                return Err("--sdk must select the SDK lib directory".to_string());
            }
            lib.parent()
                .map(PathBuf::from)
                .ok_or_else(|| "SDK root is missing".to_string())
        })
        .transpose()?;
    let sdk = match sdk_version {
        Some(version) => constraints::parse_version(&version)?,
        None => sdk_root
            .as_deref()
            .map(sdk::version)
            .transpose()?
            .unwrap_or(constraints::parse_version("3.6.2")?),
    };
    let host = hosted::normalize_host(
        &std::env::var("PUB_HOSTED_URL").unwrap_or_else(|_| "https://pub.dev".into()),
    )?;
    let cache = std::env::var_os("PUB_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".pub-cache")))
        .ok_or("set PUB_CACHE to the package cache directory")?;
    let cache = if cache.is_absolute() {
        cache
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(cache)
    };
    let root = manifest::Manifest::read(&directory, &host, true)?;
    let locked = output::read_lock(&directory, &host)?;
    let mut resolver = resolver::Resolver::new(
        hosted::Hosted::new(cache, offline, host),
        sdk.clone(),
        locked,
    );
    resolver.sdk_root = sdk_root;
    let selected = resolver.resolve(&root, &directory)?;
    for value in selected.values() {
        if let manifest::Source::Hosted(host) = &value.source {
            resolver.hosted.materialize(&value.candidate, host)?;
        }
    }
    output::write(&directory, &root, &selected, &sdk)?;
    println!("Resolved {} dependencies with DartForge.", selected.len());
    Ok(())
}
