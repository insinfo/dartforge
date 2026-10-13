//! The version profile: which `build_config` and `build_runner` rules apply
//! to the project, according to the version `pubspec.lock` resolved.
//!
//! The engine used to imitate only `build_runner` 2.4.15 with `build_config`
//! 1.1.2. Newer versions change the accepted configuration and what runs:
//!
//! | version | change | where |
//! |---|---|---|
//! | `build_config` 1.2.0 | top-level `triggers` key | `build_config.g.dart` |
//! | `build_config` 1.3.0 | `build_to` in `post_process_builders` | `builder_definition.g.dart` |
//! | `build_config` 1.3.2 | a builder or target key from another package is an error | `key_normalization.dart` |
//! | `build_runner` 2.7.0 | `run_only_if_triggered` + `triggers` | `build/build.dart`, `_allowedByTriggers` |
//! | `build_runner` 2.10.1 | trigger looked up by the full key | CHANGELOG 2.10.1 |
//! | `build_runner` 2.14.0 | `--build-filter asset:<package>/<path>` | `build_plan/build_filter.dart` |
//! | `build_runner` 2.15.3 | a duplicate builder key is an error | `builder_ordering.dart` |
//!
//! Without `build_config` in the lock (no `build_runner`: DF-BUILD-001 found a
//! `build.yaml` with builders) the 1.1.2 profile applies. Without
//! `build_runner`, `build_config` decides whether triggers apply (the key only
//! exists for `build_runner` to read).
use std::cmp::Ordering;

/// Semantic version `major.minor.patch` (the `-dev`/`+build` suffix is
/// ignored in comparisons: the engine only distinguishes published versions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// Reads `1.2.3`, `1.2.3-dev.1` or `1.2.3+4`.
    ///
    /// ```
    /// use dartforge_build::perfil::Versao;
    /// assert_eq!(Versao::ler("2.16.1"), Some(Versao(2, 16, 1)));
    /// assert_eq!(Versao::ler("2.6.0-dev.2"), Some(Versao(2, 6, 0)));
    /// assert_eq!(Versao::ler("x"), None);
    /// ```
    pub fn read(text: &str) -> Option<Version> {
        let core = text.split(['-', '+']).next()?;
        let mut parts = core.split('.').map(|p| p.parse::<u32>().ok());
        let v = Version(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(v)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// The rules that apply to a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    /// `build_config` version from the lock (`None`: missing, 1.1.2 applies).
    pub build_config: Option<Version>,
    /// `build_runner` version from the lock.
    pub build_runner: Option<Version>,
}

impl Default for Profile {
    /// The engine's historical profile: `build_runner` 2.4.15, `build_config` 1.1.2.
    fn default() -> Self {
        Profile {
            build_config: Some(Version(1, 1, 2)),
            build_runner: Some(Version(2, 4, 15)),
        }
    }
}

fn at_least(v: Option<Version>, minimum: Version) -> bool {
    v.is_some_and(|v| v.cmp(&minimum) != Ordering::Less)
}

impl Profile {
    /// The profile for the versions in the lock.
    ///
    /// ```
    /// use dartforge_build::perfil::Perfil;
    /// let p = Perfil::das_versoes(Some("1.3.3"), Some("2.16.1"));
    /// assert!(p.aceita_triggers() && p.dispara() && p.filtro_asset());
    /// let old = Perfil::das_versoes(Some("1.1.2"), Some("2.4.15"));
    /// assert!(!old.aceita_triggers() && !old.dispara());
    /// ```
    pub fn from_versions(build_config: Option<&str>, build_runner: Option<&str>) -> Profile {
        Profile {
            build_config: build_config.and_then(Version::read),
            build_runner: build_runner.and_then(Version::read),
        }
    }

    /// `build_config` ≥ 1.2.0: the top-level `triggers` key exists (otherwise
    /// `$checkKeys` rejects it like any unknown key).
    pub fn aceita_triggers(&self) -> bool {
        at_least(self.build_config, Version(1, 2, 0))
    }

    /// `build_config` ≥ 1.3.0: `post_process_builders.<x>.build_to`.
    pub fn aceita_build_to_pos(&self) -> bool {
        at_least(self.build_config, Version(1, 3, 0))
    }

    /// `build_config` ≥ 1.3.2: a builder definition or target key written
    /// with another package (`other:x` in the `build.yaml` of `p`) is an
    /// error.
    pub fn requires_package_key(&self) -> bool {
        at_least(self.build_config, Version(1, 3, 2))
    }

    /// Triggers decide whether a step with `run_only_if_triggered: true`
    /// runs: `build_runner` ≥ 2.7.0 or, without `build_runner`,
    /// `build_config` ≥ 1.2.0.
    pub fn dispara(&self) -> bool {
        match self.build_runner {
            Some(v) => v >= Version(2, 7, 0),
            None => self.aceita_triggers(),
        }
    }

    /// Between 2.7.0 and 2.10.0, `build_runner` looked up the triggers of a
    /// `p:p` builder by the short name `p` (fixed in 2.10.1).
    pub fn trigger_by_short_name(&self) -> bool {
        self.build_runner
            .is_some_and(|v| v >= Version(2, 7, 0) && v < Version(2, 10, 1))
    }

    /// `build_runner` ≥ 2.14.0: `--build-filter asset:<package>/<path>`.
    pub fn asset_filter(&self) -> bool {
        at_least(self.build_runner, Version(2, 14, 0))
    }

    /// `build_runner` ≥ 2.15.3: `findBuilderOrder` rejects a duplicate key.
    pub fn rejects_duplicate_key(&self) -> bool {
        at_least(self.build_runner, Version(2, 15, 3))
    }

    /// Short text for reports (`build_runner 2.16.1, build_config 1.3.3`).
    pub fn text(&self) -> String {
        let v = |x: Option<Version>| x.map_or_else(|| "missing".to_string(), |v| v.to_string());
        format!(
            "build_runner {}, build_config {}",
            v(self.build_runner),
            v(self.build_config)
        )
    }
}
