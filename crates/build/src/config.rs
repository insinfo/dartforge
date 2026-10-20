//! `pubspec.yaml`, `pubspec.lock` and `build.yaml`, read the way the locked
//! `build_config` reads them ([`crate::profile`]): 1.1.2 by default, 1.2.0
//! with the top-level `triggers` key, 1.3.x with `build_to` in
//! `post_process_builders` and the definition key tied to the package.
//!
//! `build.yaml` follows the `$checkKeys` of `build_config`'s `*.g.dart`
//! files: an unknown key is an error, a missing required field is an error, a
//! wrong type is an error. Builder and target keys are normalized like
//! `key_normalization.dart`.
use crate::profile::Profile;
use crate::value::{Map, Value};
use std::path::Path;
use yaml_rust2::{Yaml, YamlLoader};

/// Loads a YAML document (the first one; empty = `Null`).
pub fn load_yaml(text: &str, origin: &str) -> Result<Yaml, String> {
    let docs =
        YamlLoader::load_from_str(text).map_err(|e| format!("{origin}: invalid YAML: {e}"))?;
    Ok(docs.into_iter().next().unwrap_or(Yaml::Null))
}

fn string_keys(y: &Yaml) -> Vec<String> {
    match y {
        Yaml::Hash(h) => h
            .keys()
            .filter_map(|k| k.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------- pubspec

#[derive(Debug, Clone, Default)]
pub struct Pubspec {
    pub name: Option<String>,
    /// Keys of `dependencies`, in file order.
    pub dependencies: Vec<String>,
    pub dev_dependencies: Vec<String>,
}

impl Pubspec {
    pub fn read(dir: &Path) -> Result<Pubspec, String> {
        let path = dir.join("pubspec.yaml");
        let text = std::fs::read_to_string(&path).map_err(|_| {
            format!(
                "Unable to generate package graph, no `{}` found.",
                path.display()
            )
        })?;
        Self::from_text(&text, &path.display().to_string())
    }

    pub fn from_text(text: &str, origin: &str) -> Result<Pubspec, String> {
        let y = load_yaml(text, origin)?;
        Ok(Pubspec {
            name: y["name"].as_str().map(str::to_string),
            dependencies: string_keys(&y["dependencies"]),
            dev_dependencies: string_keys(&y["dev_dependencies"]),
        })
    }
}

/// Dependency type (`package_graph.dart`, `DependencyType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyType {
    Github,
    Path,
    Hosted,
}

/// One entry of `pubspec.lock`.
#[derive(Debug, Clone)]
pub struct LockedPackage {
    pub kind: DependencyType,
    pub version: String,
    /// `description.url` of a `hosted` package (the server it came from).
    pub server: Option<String>,
}

impl LockedPackage {
    /// The package is the one published on pub.dev with this version —
    /// immutable, so the version identifies the code. A fork (`path`, `git`,
    /// another server) with the same version is not (DF-BUILD-022).
    pub fn is_from_pub_dev(&self) -> bool {
        self.kind == DependencyType::Hosted
            && self.server.as_deref().is_some_and(|u| {
                matches!(
                    u.trim_end_matches('/'),
                    "https://pub.dev" | "https://pub.dartlang.org"
                )
            })
    }
}

pub fn read_lock(text: &str, origin: &str) -> Result<Vec<(String, LockedPackage)>, String> {
    let y = load_yaml(text, origin)?;
    let Yaml::Hash(packages) = &y["packages"] else {
        return Err(format!("{origin}: no `packages`"));
    };
    let mut v = Vec::new();
    for (k, p) in packages {
        let Some(name) = k.as_str() else { continue };
        let kind = match p["source"].as_str() {
            Some("git") => DependencyType::Github,
            Some("hosted") => DependencyType::Hosted,
            Some("path") | Some("sdk") => DependencyType::Path,
            other => return Err(format!("Unable to determine dependency type:\n{other:?}")),
        };
        let version = p["version"].as_str().unwrap_or_default().to_string();
        let server = p["description"]["url"].as_str().map(str::to_string);
        v.push((
            name.to_string(),
            LockedPackage {
                kind,
                version,
                server,
            },
        ));
    }
    Ok(v)
}

// ------------------------------------------------------- key normalization

pub fn builder_definition_key(key: &str, package: &str) -> String {
    normalize_definition(&key.replacen('|', ":", 1), package)
}

/// `_normalizeDefinition` of `build_config` ≥ 1.3.2: the key `other:x`
/// defined in package `p` is an error.
fn check_owner(normalized_key: &str, package: &str) -> Result<(), String> {
    if normalized_key.starts_with(&format!("{package}:")) {
        Ok(())
    } else {
        Err(format!(
            "Builder key \"{normalized_key}\" does not belong to package \"{package}\"."
        ))
    }
}

pub fn builder_usage_key(key: &str, package: &str) -> String {
    normalize_usage(&key.replacen('|', ":", 1), package)
}

pub fn target_definition_key(key: &str, package: &str) -> String {
    if key == "$default" {
        format!("{package}:{package}")
    } else {
        normalize_definition(key, package)
    }
}

pub fn target_usage_key(key: &str, package: &str) -> String {
    match key {
        "$default" | ":$default" | "$default:$default" => format!("{package}:{package}"),
        _ => normalize_usage(key, package),
    }
}

fn normalize_usage(name: &str, package: &str) -> String {
    if name.starts_with(':') {
        return format!("{package}{name}");
    }
    if !name.contains(':') {
        return format!("{name}:{name}");
    }
    name.to_string()
}

fn normalize_definition(name: &str, package: &str) -> String {
    if name.starts_with(':') {
        return format!("{package}{name}");
    }
    if !name.contains(':') {
        return format!("{package}:{name}");
    }
    name.to_string()
}

// ------------------------------------------------------------- build.yaml

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoApply {
    None,
    Dependents,
    AllPackages,
    RootPackage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTo {
    Source,
    Cache,
}

/// `InputSet`: `None` = any path.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InputSet {
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

impl InputSet {
    pub fn canonical_text(&self) -> String {
        let l = |v: &Option<Vec<String>>| match v {
            None => "-".to_string(),
            Some(v) => {
                Value::List(v.iter().map(|s| Value::Text(s.clone())).collect()).canonical_text()
            }
        };
        format!(
            "InputSet(include: {}, exclude: {})",
            l(&self.include),
            l(&self.exclude)
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Defaults {
    /// `None` = `InputSet.anything` (identical): it does not appear in
    /// `build.dart`.
    pub generate_for: Option<InputSet>,
    pub options: Map,
    pub dev_options: Map,
    pub release_options: Map,
}

#[derive(Debug, Clone)]
pub struct BuilderDefinition {
    pub key: String,
    pub package: String,
    pub factories: Vec<String>,
    pub import: String,
    pub extensions: Vec<(String, Vec<String>)>,
    pub auto_apply: AutoApply,
    pub required_inputs: Vec<String>,
    pub runs_before: Vec<String>,
    pub applies_builders: Vec<String>,
    pub optional: bool,
    pub build_to: BuildTo,
    pub defaults: Defaults,
}

#[derive(Debug, Clone)]
pub struct PostProcessDefinition {
    pub key: String,
    pub package: String,
    pub factory: String,
    pub import: String,
    /// `build_to` (`build_config` ≥ 1.3.0; before that, always `cache`).
    pub build_to: BuildTo,
    pub defaults: Defaults,
}

#[derive(Debug, Clone, Default)]
pub struct TargetBuilderConfig {
    pub enabled: bool,
    pub generate_for: Option<InputSet>,
    pub options: Map,
    pub dev_options: Map,
    pub release_options: Map,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub key: String,
    pub package: String,
    pub auto_apply_builders: bool,
    pub builders: Vec<(String, TargetBuilderConfig)>,
    pub dependencies: Vec<String>,
    pub sources: InputSet,
}

impl Target {
    pub fn cfg(&self, key: &str) -> Option<&TargetBuilderConfig> {
        self.builders.iter().find(|(k, _)| k == key).map(|(_, c)| c)
    }
}

#[derive(Debug, Clone, Default)]
pub struct GlobalBuilderConfig {
    pub options: Map,
    pub dev_options: Map,
    pub release_options: Map,
    pub runs_before: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub package: String,
    pub builders: Vec<BuilderDefinition>,
    pub post_process_builders: Vec<PostProcessDefinition>,
    pub targets: Vec<Target>,
    pub global: Vec<(String, GlobalBuilderConfig)>,
    pub additional_public: Vec<String>,
    /// `triggers` (`build_config` ≥ 1.2.0): by builder name, **not
    /// normalized**, with the value as the YAML had it. Validity and meaning
    /// belong to `build_runner` ([`crate::triggers`]).
    pub triggers: Vec<(String, Value)>,
}

impl BuildConfig {
    /// `BuildConfig.useDefault`.
    pub fn pattern(package: &str, dependencies: &[String]) -> BuildConfig {
        BuildConfig {
            package: package.to_string(),
            builders: Vec::new(),
            post_process_builders: Vec::new(),
            targets: vec![Target {
                key: format!("{package}:{package}"),
                package: package.to_string(),
                auto_apply_builders: true,
                builders: Vec::new(),
                dependencies: dependencies
                    .iter()
                    .map(|d| target_usage_key(d, package))
                    .collect(),
                sources: InputSet::default(),
            }],
            global: Vec::new(),
            additional_public: Vec::new(),
            triggers: Vec::new(),
        }
    }

    pub fn global_for(&self, key: &str) -> Option<&GlobalBuilderConfig> {
        self.global.iter().find(|(k, _)| k == key).map(|(_, c)| c)
    }

    /// `BuildConfig.fromBuildConfigDir`: the directory's `build.yaml`, or the
    /// default. Read as `build_config` 1.1.2 does;
    /// [`BuildConfig::from_dir_with_profile`] picks the version.
    ///
    /// # Errors
    /// A `build.yaml` that exists and cannot be read, or that `build_config`
    /// rejects.
    pub fn from_dir(
        package: &str,
        dependencies: &[String],
        dir: &Path,
    ) -> Result<BuildConfig, String> {
        Self::from_dir_with_profile(package, dependencies, dir, Profile::default())
    }

    /// [`BuildConfig::from_dir`] with the rules of the `build_config` of
    /// `profile`.
    ///
    /// # Errors
    /// As [`BuildConfig::from_dir`].
    pub fn from_dir_with_profile(
        package: &str,
        dependencies: &[String],
        dir: &Path,
        profile: Profile,
    ) -> Result<BuildConfig, String> {
        let path = dir.join("build.yaml");
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::from_text_with_profile(
                package,
                dependencies,
                &text,
                &path.display().to_string(),
                profile,
            ),
            // Only absence means the default (`fromBuildConfigDir` checks
            // `exists`); a `build.yaml` that exists and cannot be read
            // (permissions, bytes that are not UTF-8, a directory with that
            // name) is an error, not an empty configuration (DF-BUILD-017).
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::pattern(package, dependencies))
            }
            Err(e) => Err(format!("could not read {}: {e}", path.display())),
        }
    }

    /// `BuildConfig.parse` of `build_config` 1.1.2.
    ///
    /// ```
    /// use dartforge_build::config::BuildConfig;
    /// let c = BuildConfig::from_text("p", &[], "targets:\n  $default: {}\n", "build.yaml").unwrap();
    /// assert_eq!(c.targets[0].key, "p:p");
    /// // `triggers` only exists from build_config 1.2.0 on.
    /// assert!(BuildConfig::from_text("p", &[], "triggers: {}\n", "build.yaml").is_err());
    /// ```
    ///
    /// # Errors
    /// Invalid YAML, unknown key, missing required field or wrong type, like
    /// `$checkKeys`.
    pub fn from_text(
        package: &str,
        dependencies: &[String],
        text: &str,
        origin: &str,
    ) -> Result<BuildConfig, String> {
        Self::from_text_with_profile(package, dependencies, text, origin, Profile::default())
    }

    /// `BuildConfig.parse` of the `build_config` of `profile`.
    ///
    /// ```
    /// use dartforge_build::config::BuildConfig;
    /// use dartforge_build::profile::Profile;
    /// let newer = Profile::from_versions(Some("1.2.0"), Some("2.7.0"));
    /// let t = "triggers:\n  p:b:\n    - annotation A\n";
    /// let c = BuildConfig::from_text_with_profile("p", &[], t, "build.yaml", newer).unwrap();
    /// assert_eq!(c.triggers[0].0, "p:b");
    /// ```
    ///
    /// # Errors
    /// As [`BuildConfig::from_text`], with the keys of that version.
    pub fn from_text_with_profile(
        package: &str,
        dependencies: &[String],
        text: &str,
        origin: &str,
        profile: Profile,
    ) -> Result<BuildConfig, String> {
        let y = load_yaml(text, origin)?;
        let ctx = Ctx {
            package,
            deps: dependencies,
            origin,
            profile,
        };
        ctx.config(&y)
    }
}

struct Ctx<'a> {
    package: &'a str,
    deps: &'a [String],
    origin: &'a str,
    profile: Profile,
}

type R<T> = Result<T, String>;

impl Ctx<'_> {
    fn error(&self, at: &str, msg: &str) -> String {
        format!("{}: {at}: {msg}", self.origin)
    }

    fn map<'y>(&self, y: &'y Yaml, at: &str) -> R<Option<&'y yaml_rust2::yaml::Hash>> {
        match y {
            Yaml::Null | Yaml::BadValue => Ok(None),
            Yaml::Hash(h) => Ok(Some(h)),
            _ => Err(self.error(at, "expected a map")),
        }
    }

    fn check_keys(&self, y: &Yaml, at: &str, allowed: &[&str], required: &[&str]) -> R<()> {
        let Some(h) = self.map(y, at)? else {
            return Ok(());
        };
        let mut unknown = Vec::new();
        for k in h.keys() {
            let k = k
                .as_str()
                .ok_or_else(|| self.error(at, "key is not a string"))?;
            if !allowed.contains(&k) {
                unknown.push(k.to_string());
            }
        }
        if !unknown.is_empty() {
            return Err(self.error(
                at,
                &format!(
                    "Unrecognized keys: [{}]; supported keys: [{}]",
                    unknown.join(", "),
                    allowed.join(", ")
                ),
            ));
        }
        for o in required {
            match &y[*o] {
                Yaml::BadValue => {
                    return Err(self.error(at, &format!("Required keys are missing: {o}.")));
                }
                Yaml::Null => {
                    return Err(self.error(
                        at,
                        &format!("These keys had `null` values, which is not allowed: [{o}]"),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn string(&self, y: &Yaml, at: &str) -> R<Option<String>> {
        match y {
            Yaml::Null | Yaml::BadValue => Ok(None),
            Yaml::String(s) => Ok(Some(s.clone())),
            _ => Err(self.error(at, "expected a string")),
        }
    }

    fn boolean(&self, y: &Yaml, at: &str) -> R<Option<bool>> {
        match y {
            Yaml::Null | Yaml::BadValue => Ok(None),
            Yaml::Boolean(b) => Ok(Some(*b)),
            _ => Err(self.error(at, "expected a bool")),
        }
    }

    fn string_list(&self, y: &Yaml, at: &str) -> R<Option<Vec<String>>> {
        match y {
            Yaml::Null | Yaml::BadValue => Ok(None),
            Yaml::Array(a) => a
                .iter()
                .map(|e| {
                    e.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| self.error(at, "expected a list of strings"))
                })
                .collect::<R<Vec<_>>>()
                .map(Some),
            _ => Err(self.error(at, "expected a list")),
        }
    }

    fn options(&self, y: &Yaml, at: &str) -> R<Map> {
        match self.map(y, at)? {
            None => Ok(Map::default()),
            Some(h) => {
                let mut m = Map::default();
                for (k, v) in h {
                    let k = k
                        .as_str()
                        .ok_or_else(|| self.error(at, "option key is not a string"))?;
                    m.insert(
                        Value::Text(k.to_string()),
                        Value::from_yaml(v).map_err(|e| self.error(at, &e))?,
                    );
                }
                Ok(m)
            }
        }
    }

    /// `InputSet.fromJson`: a list = `include`; a map with `include`/`exclude`.
    fn input_set(&self, y: &Yaml, at: &str) -> R<Option<InputSet>> {
        let s = match y {
            Yaml::Null | Yaml::BadValue => return Ok(None),
            Yaml::Array(a) => {
                if a.iter().any(|e| matches!(e, Yaml::Null)) {
                    return Err(self.error(at, "Include globs must not be empty"));
                }
                InputSet {
                    include: self.string_list(y, at)?,
                    exclude: None,
                }
            }
            Yaml::Hash(_) => {
                self.check_keys(y, at, &["include", "exclude"], &[])?;
                InputSet {
                    include: self.string_list(&y["include"], at)?,
                    exclude: self.string_list(&y["exclude"], at)?,
                }
            }
            _ => return Err(self.error(at, "Expected a Map or a List")),
        };
        if s.include
            .as_ref()
            .is_some_and(|v| v.iter().any(String::is_empty))
        {
            return Err(self.error(at, "Include globs must not be empty"));
        }
        if s.exclude
            .as_ref()
            .is_some_and(|v| v.iter().any(String::is_empty))
        {
            return Err(self.error(at, "Exclude globs must not be empty"));
        }
        Ok(Some(s))
    }

    fn defaults(&self, y: &Yaml, at: &str) -> R<Defaults> {
        self.check_keys(
            y,
            at,
            &["generate_for", "options", "dev_options", "release_options"],
            &[],
        )?;
        Ok(Defaults {
            generate_for: self.input_set(&y["generate_for"], at)?,
            options: self.options(&y["options"], at)?,
            dev_options: self.options(&y["dev_options"], at)?,
            release_options: self.options(&y["release_options"], at)?,
        })
    }

    fn builder(&self, key: &str, y: &Yaml) -> R<BuilderDefinition> {
        let at = &format!("builders.{key}");
        self.check_keys(
            y,
            at,
            &[
                "builder_factories",
                "import",
                "build_extensions",
                "target",
                "auto_apply",
                "required_inputs",
                "runs_before",
                "applies_builders",
                "is_optional",
                "build_to",
                "defaults",
            ],
            &["builder_factories", "import", "build_extensions"],
        )?;
        let factories = self
            .string_list(&y["builder_factories"], at)?
            .unwrap_or_default();
        if factories.is_empty() {
            return Err(self.error(at, "builderFactories: Must have at least one value."));
        }
        let mut extensions = Vec::new();
        let Some(h) = self.map(&y["build_extensions"], at)? else {
            return Err(self.error(at, "missing build_extensions"));
        };
        for (k, v) in h {
            let k = k
                .as_str()
                .ok_or_else(|| self.error(at, "extension is not a string"))?
                .to_string();
            let outputs = self
                .string_list(v, at)?
                .ok_or_else(|| self.error(at, "missing output extensions"))?;
            if outputs.contains(&k) {
                return Err(self.error(
                    at,
                    "May not overwrite an input, the output extensions must not contain the input extension",
                ));
            }
            extensions.push((k, outputs));
        }
        let auto_apply = match self.string(&y["auto_apply"], at)?.as_deref() {
            None | Some("none") => AutoApply::None,
            Some("dependents") => AutoApply::Dependents,
            Some("all_packages") => AutoApply::AllPackages,
            Some("root_package") => AutoApply::RootPackage,
            Some(o) => return Err(self.error(at, &format!("unknown auto_apply: {o}"))),
        };
        let build_to = match self.string(&y["build_to"], at)?.as_deref() {
            None | Some("cache") => BuildTo::Cache,
            Some("source") => BuildTo::Source,
            Some(o) => return Err(self.error(at, &format!("unknown build_to: {o}"))),
        };
        let usage = |v: Option<Vec<String>>| {
            v.unwrap_or_default()
                .iter()
                .map(|b| builder_usage_key(b, self.package))
                .collect::<Vec<_>>()
        };
        let normalized_key = builder_definition_key(key, self.package);
        if self.profile.requires_package_key() {
            check_owner(&normalized_key, self.package).map_err(|e| self.error(at, &e))?;
        }
        Ok(BuilderDefinition {
            key: normalized_key,
            package: self.package.to_string(),
            factories,
            import: self.string(&y["import"], at)?.unwrap_or_default(),
            extensions,
            auto_apply,
            required_inputs: self
                .string_list(&y["required_inputs"], at)?
                .unwrap_or_default(),
            runs_before: usage(self.string_list(&y["runs_before"], at)?),
            applies_builders: usage(self.string_list(&y["applies_builders"], at)?),
            optional: self.boolean(&y["is_optional"], at)?.unwrap_or(false),
            build_to,
            defaults: match &y["defaults"] {
                Yaml::Null | Yaml::BadValue => Defaults::default(),
                d => self.defaults(d, at)?,
            },
        })
    }

    fn post_process(&self, key: &str, y: &Yaml) -> R<PostProcessDefinition> {
        let at = &format!("post_process_builders.{key}");
        let newer = self.profile.accepts_post_process_build_to();
        let allowed: &[&str] = if newer {
            &[
                "builder_factory",
                "import",
                "input_extensions",
                "target",
                "build_to",
                "defaults",
            ]
        } else {
            &[
                "builder_factory",
                "import",
                "input_extensions",
                "target",
                "defaults",
            ]
        };
        self.check_keys(y, at, allowed, &["builder_factory", "import"])?;
        let build_to = match self.string(&y["build_to"], at)?.as_deref() {
            None | Some("cache") => BuildTo::Cache,
            Some("source") => BuildTo::Source,
            Some(o) => return Err(self.error(at, &format!("unknown build_to: {o}"))),
        };
        let normalized_key = builder_definition_key(key, self.package);
        if self.profile.requires_package_key() {
            check_owner(&normalized_key, self.package).map_err(|e| self.error(at, &e))?;
        }
        Ok(PostProcessDefinition {
            key: normalized_key,
            package: self.package.to_string(),
            factory: self.string(&y["builder_factory"], at)?.unwrap_or_default(),
            import: self.string(&y["import"], at)?.unwrap_or_default(),
            build_to,
            defaults: match &y["defaults"] {
                Yaml::Null | Yaml::BadValue => Defaults::default(),
                d => self.defaults(d, at)?,
            },
        })
    }

    fn target(&self, key: &str, y: &Yaml) -> R<Target> {
        let at = &format!("targets.{key}");
        if !matches!(y, Yaml::Hash(_)) {
            return Err(self.error(at, "expected a map"));
        }
        self.check_keys(
            y,
            at,
            &["auto_apply_builders", "builders", "dependencies", "sources"],
            &[],
        )?;
        let mut builders = Vec::new();
        if let Some(h) = self.map(&y["builders"], at)? {
            for (k, v) in h {
                let k = k
                    .as_str()
                    .ok_or_else(|| self.error(at, "builder key is not a string"))?;
                let o = &format!("{at}.builders.{k}");
                if !matches!(v, Yaml::Hash(_)) {
                    return Err(self.error(o, "expected a map"));
                }
                self.check_keys(
                    v,
                    o,
                    &[
                        "enabled",
                        "generate_for",
                        "options",
                        "dev_options",
                        "release_options",
                    ],
                    &[],
                )?;
                let cfg = TargetBuilderConfig {
                    enabled: self.boolean(&v["enabled"], o)?.unwrap_or(true),
                    generate_for: self.input_set(&v["generate_for"], o)?,
                    options: self.options(&v["options"], o)?,
                    dev_options: self.options(&v["dev_options"], o)?,
                    release_options: self.options(&v["release_options"], o)?,
                };
                let builder_key = builder_usage_key(k, self.package);
                if let Some(p) = builders
                    .iter_mut()
                    .find(|(c, _): &&mut (String, TargetBuilderConfig)| *c == builder_key)
                {
                    p.1 = cfg;
                } else {
                    builders.push((builder_key, cfg));
                }
            }
        }
        let dependencies = match self.string_list(&y["dependencies"], at)? {
            Some(v) => v,
            None => self.deps.to_vec(),
        };
        let target_key = target_definition_key(key, self.package);
        if self.profile.requires_package_key() {
            check_owner(&target_key, self.package).map_err(|e| self.error(at, &e))?;
        }
        Ok(Target {
            key: target_key,
            package: self.package.to_string(),
            auto_apply_builders: self.boolean(&y["auto_apply_builders"], at)?.unwrap_or(true),
            builders,
            dependencies: dependencies
                .iter()
                .map(|d| target_usage_key(d, self.package))
                .collect(),
            sources: self.input_set(&y["sources"], at)?.unwrap_or_default(),
        })
    }

    fn config(&self, y: &Yaml) -> R<BuildConfig> {
        let empty = Yaml::Hash(Default::default());
        let y = if matches!(y, Yaml::Null) { &empty } else { y };
        if !matches!(y, Yaml::Hash(_)) {
            return Err(self.error("build.yaml", "expected a map"));
        }
        let mut allowed = vec![
            "builders",
            "post_process_builders",
            "targets",
            "global_options",
            "additional_public_assets",
        ];
        if self.profile.accepts_triggers() {
            allowed.push("triggers");
        }
        self.check_keys(y, "build.yaml", &allowed, &[])?;
        // `triggersByBuilder`: `(v as Map?)?.map((k, e) => MapEntry(k as
        // String, e as Object))` — the value cannot be null.
        let mut triggers = Vec::new();
        if self.profile.accepts_triggers()
            && let Some(h) = self.map(&y["triggers"], "triggers")?
        {
            for (k, v) in h {
                let k = k
                    .as_str()
                    .ok_or_else(|| self.error("triggers", "key is not a string"))?;
                if matches!(v, Yaml::Null) {
                    return Err(self.error(
                        &format!("triggers.{k}"),
                        "null value (expected a list of triggers)",
                    ));
                }
                let value = Value::from_yaml(v).map_err(|e| self.error("triggers", &e))?;
                triggers.retain(|(x, _): &(String, Value)| x != k);
                triggers.push((k.to_string(), value));
            }
        }
        let mut targets = Vec::new();
        match self.map(&y["targets"], "targets")? {
            None => targets = BuildConfig::pattern(self.package, self.deps).targets,
            Some(h) => {
                for (k, v) in h {
                    let k = k
                        .as_str()
                        .ok_or_else(|| self.error("targets", "key is not a string"))?;
                    targets.push(self.target(k, v)?);
                }
                let default = format!("{0}:{0}", self.package);
                if !targets.iter().any(|a| a.key == default) {
                    return Err(self.error(
                        "targets",
                        &format!(
                            "Must specify a target with the name `{}` or `$default`.",
                            self.package
                        ),
                    ));
                }
            }
        }
        let mut global = Vec::new();
        if let Some(h) = self.map(&y["global_options"], "global_options")? {
            for (k, v) in h {
                let k = k
                    .as_str()
                    .ok_or_else(|| self.error("global_options", "key is not a string"))?;
                let o = &format!("global_options.{k}");
                if !matches!(v, Yaml::Hash(_)) {
                    return Err(self.error(o, "expected a map"));
                }
                self.check_keys(
                    v,
                    o,
                    &["options", "dev_options", "release_options", "runs_before"],
                    &[],
                )?;
                global.push((
                    builder_usage_key(k, self.package),
                    GlobalBuilderConfig {
                        options: self.options(&v["options"], o)?,
                        dev_options: self.options(&v["dev_options"], o)?,
                        release_options: self.options(&v["release_options"], o)?,
                        runs_before: self
                            .string_list(&v["runs_before"], o)?
                            .unwrap_or_default()
                            .iter()
                            .map(|b| builder_usage_key(b, self.package))
                            .collect(),
                    },
                ));
            }
        }
        let mut builders = Vec::new();
        if let Some(h) = self.map(&y["builders"], "builders")? {
            for (k, v) in h {
                let k = k
                    .as_str()
                    .ok_or_else(|| self.error("builders", "key is not a string"))?;
                let b = self.builder(k, v)?;
                builders.retain(|x: &BuilderDefinition| x.key != b.key);
                builders.push(b);
            }
        }
        let mut post = Vec::new();
        if let Some(h) = self.map(&y["post_process_builders"], "post_process_builders")? {
            for (k, v) in h {
                let k = k
                    .as_str()
                    .ok_or_else(|| self.error("post_process_builders", "key is not a string"))?;
                let p = self.post_process(k, v)?;
                post.retain(|x: &PostProcessDefinition| x.key != p.key);
                post.push(p);
            }
        }
        Ok(BuildConfig {
            package: self.package.to_string(),
            builders,
            post_process_builders: post,
            targets,
            global,
            additional_public: self
                .string_list(&y["additional_public_assets"], "additional_public_assets")?
                .unwrap_or_default(),
            triggers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_normalization() {
        assert_eq!(builder_definition_key("x", "p"), "p:x");
        assert_eq!(builder_definition_key("q|x", "p"), "q:x");
        assert_eq!(builder_usage_key("ngdart", "p"), "ngdart:ngdart");
        assert_eq!(builder_usage_key(":x", "p"), "p:x");
        assert_eq!(
            builder_usage_key("ngdart|placeholder_cleanup", "p"),
            "ngdart:placeholder_cleanup"
        );
        assert_eq!(target_definition_key("$default", "p"), "p:p");
        assert_eq!(target_usage_key(":$default", "p"), "p:p");
        assert_eq!(target_usage_key("dep", "p"), "dep:dep");
    }

    #[test]
    fn unreadable_build_yaml_is_an_error() {
        let dir = std::env::temp_dir().join(format!("dartforge-build-yaml-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("build.yaml"), [0xff, 0xfe, 0x00]).unwrap();
        let r = BuildConfig::from_dir("p", &[], &dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(
            r.is_err(),
            "unreadable build.yaml became the default configuration"
        );
        let empty = std::env::temp_dir().join(format!(
            "dartforge-build-yaml-missing-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&empty).unwrap();
        let r = BuildConfig::from_dir("p", &[], &empty);
        std::fs::remove_dir_all(&empty).unwrap();
        assert!(r.is_ok());
    }

    #[test]
    fn sass_builder_build_yaml() {
        let t = r#"
builders:
  sass_builder:
    import: "package:sass_builder/sass_builder.dart"
    builder_factories: ["sassBuilder"]
    auto_apply: dependents
    build_extensions:
      .scss: [".css", ".css.map"]
      .sass: [".css", ".css.map"]
    applies_builders:
      - sass_builder:sass_source_cleanup
    defaults:
      release_options:
        outputStyle: compressed
        sourceMaps: false
      dev_options:
        sourceMaps: true
post_process_builders:
  sass_source_cleanup:
    import: "package:sass_builder/sass_builder.dart"
    builder_factory: "sassSourceCleanup"
    defaults:
      release_options:
        enabled: true
"#;
        let c = BuildConfig::from_text("sass_builder", &["sass".into()], t, "test").unwrap();
        let b = &c.builders[0];
        assert_eq!(b.key, "sass_builder:sass_builder");
        assert_eq!(b.auto_apply, AutoApply::Dependents);
        assert_eq!(b.build_to, BuildTo::Cache);
        assert_eq!(
            b.defaults.release_options.canonical_text(),
            r#"{"outputStyle": "compressed", "sourceMaps": false}"#
        );
        assert_eq!(
            c.post_process_builders[0].key,
            "sass_builder:sass_source_cleanup"
        );
        assert_eq!(c.targets[0].key, "sass_builder:sass_builder");
        assert_eq!(c.targets[0].dependencies, vec!["sass:sass"]);
    }

    #[test]
    fn keys_by_profile() {
        let old = Profile::default();
        let p12 = Profile::from_versions(Some("1.2.0"), Some("2.7.0"));
        let p13 = Profile::from_versions(Some("1.3.3"), Some("2.16.1"));
        let trig = "triggers:\n  p:b: [annotation A]\n";
        assert!(BuildConfig::from_text_with_profile("p", &[], trig, "t", old).is_err());
        assert!(BuildConfig::from_text_with_profile("p", &[], trig, "t", p12).is_ok());
        // `e as Object`: a null value is an error.
        assert!(
            BuildConfig::from_text_with_profile("p", &[], "triggers:\n  p:b:\n", "t", p12).is_err()
        );
        let post = "post_process_builders:\n  x:\n    import: a.dart\n    builder_factory: f\n    build_to: source\n";
        assert!(BuildConfig::from_text_with_profile("p", &[], post, "t", p12).is_err());
        let c = BuildConfig::from_text_with_profile("p", &[], post, "t", p13).unwrap();
        assert_eq!(c.post_process_builders[0].build_to, BuildTo::Source);
        // A key from another package: accepted up to 1.3.1, an error in 1.3.2.
        let other = "builders:\n  q:x:\n    import: a.dart\n    builder_factories: [f]\n    build_extensions: {.a: [.b]}\n";
        assert!(BuildConfig::from_text_with_profile("p", &[], other, "t", p12).is_ok());
        assert!(BuildConfig::from_text_with_profile("p", &[], other, "t", p13).is_err());
    }

    #[test]
    fn unknown_key_is_an_error() {
        assert!(
            BuildConfig::from_text("p", &[], "targets:\n  $default:\n    srcs: []\n", "t").is_err()
        );
        assert!(BuildConfig::from_text("p", &[], "targets:\n  other: {}\n", "t").is_err());
        assert!(BuildConfig::from_text("p", &[], "", "t").is_ok());
    }
}
