//! Native package resolution and cache population; no Dart/Pub subprocess delegation.
mod constraints;
mod hosted;
mod manifest;
mod output;
mod resolver;
#[cfg(test)]
mod tests;
use std::{ffi::OsString, path::PathBuf};

/// Resolves the supported path and hosted package sources and writes Pub artifacts.
pub(crate) fn run(args: &[OsString]) -> Result<(), String> {
    let usage =
        "usage: dartforge pub get [--offline] [--directory <dir>] [--sdk-version <version>]";
    let mut directory = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut offline = false;
    let mut sdk = "3.6.2".to_string();
    let mut index = 0;
    while index < args.len() {
        match args[index]
            .to_str()
            .ok_or("pub get arguments must be UTF-8")?
        {
            "--offline" => offline = true,
            "--directory" | "--sdk-version" => {
                let option = args[index].to_str().unwrap();
                index += 1;
                let value = args.get(index).ok_or(usage)?;
                if option == "--directory" {
                    directory = PathBuf::from(value);
                } else {
                    sdk = value.to_str().ok_or("SDK version must be UTF-8")?.into();
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
    let sdk = constraints::parse_version(&sdk)?;
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
