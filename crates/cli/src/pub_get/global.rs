//! `dart pub global`: packages activated in `PUB_CACHE/global_packages`.
use super::{Failure, Settings};

/// `GlobalPackages.repairActivatedPackages`: the names reactivated and the
/// names that failed.
pub(crate) fn repair_activated(settings: &Settings) -> Result<(Vec<String>, Vec<String>), Failure> {
    let directory = settings.cache_root.join("global_packages");
    if std::fs::read_dir(&directory).is_ok_and(|mut entries| entries.next().is_some()) {
        return Err("pub cache repair: reactivating global packages is not supported yet".into());
    }
    Ok((Vec::new(), Vec::new()))
}
