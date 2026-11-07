//! Immutable compiler declarations, independent of the host process environment.
use crate::sdk::SdkLayout;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Backend whose declaration and integer rules apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentTarget {
    NativeJit,
    NativeAot,
    JavaScript,
}

/// Parsed integer before a backend prints or folds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvironmentInteger {
    Native(i64),
    JavaScript(f64),
}

impl EnvironmentInteger {
    /// Value representable by the shared constant evaluator without losing bits.
    ///
    /// ```
    /// use dartforge_elements::environment::EnvironmentInteger;
    /// assert_eq!(EnvironmentInteger::Native(-1).exact_integer(), Some(-1));
    /// assert_eq!(EnvironmentInteger::JavaScript(42.0).exact_integer(), Some(42));
    /// ```
    pub fn exact_integer(self) -> Option<i64> {
        match self {
            Self::Native(value) => Some(value),
            Self::JavaScript(value)
                if value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0 =>
            {
                Some(value as i64)
            }
            _ => None,
        }
    }
}

/// One compilation's complete known declarations; analyzer loads keep `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationEnvironment {
    target: EnvironmentTarget,
    values: BTreeMap<String, String>,
    runtime_values: BTreeMap<String, String>,
}

impl CompilationEnvironment {
    /// Resolves ordered `name=value` definitions and the selected SDK profile.
    /// Reserved library declarations always follow the target, and duplicates
    /// use the last value. Native JIT ignores definitions without `=`; JS rejects
    /// them. An absent name remains distinct from a declared `"false"`.
    ///
    /// # Errors
    /// A malformed definition on JavaScript, or an empty name outside native JIT.
    ///
    /// ```
    /// use dartforge_elements::{SdkLayout, environment::{CompilationEnvironment, EnvironmentTarget}};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::JavaScript,
    ///     &SdkLayout::default(), false, &["flag=first".into(), "flag=last".into()]).unwrap();
    /// assert_eq!(env.get("flag"), Some("last"));
    /// ```
    pub fn new(
        target: EnvironmentTarget,
        sdk: &SdkLayout,
        product: bool,
        definitions: &[String],
    ) -> Result<Self, String> {
        let mut values = BTreeMap::new();
        for definition in definitions {
            let Some((name, value)) = definition.split_once('=') else {
                if target == EnvironmentTarget::JavaScript {
                    return Err(format!(
                        "environment definition requires name=value: {definition}"
                    ));
                }
                continue;
            };
            if name.is_empty() {
                if target == EnvironmentTarget::NativeJit {
                    continue;
                }
                return Err("environment definition requires a nonempty name".into());
            }
            if !name.starts_with("dart.library.") {
                values.insert(name.to_string(), value.to_string());
            }
        }
        for (name, library) in &sdk.libraries {
            if library.supported
                && (target == EnvironmentTarget::JavaScript || !name.starts_with('_'))
                && !(name == "mirrors" && product && target != EnvironmentTarget::JavaScript)
            {
                values.insert(format!("dart.library.{name}"), "true".into());
            }
        }
        if target != EnvironmentTarget::JavaScript {
            values
                .entry("dart.vm.product".into())
                .or_insert_with(|| product.to_string());
            values.insert("dartforge.compiler".into(), "true".into());
            if target == EnvironmentTarget::NativeAot {
                values.insert("dart.isVM".into(), "true".into());
            } else {
                values
                    .entry("dart.isVM".into())
                    .or_insert_with(|| "true".into());
                values
                    .entry("dart.developer.timeline".into())
                    .or_insert_with(|| "true".into());
            }
        }
        // Native runtime lookup is not the CFE's constant environment. The
        // product VM keeps mirrors=false, and JIT embedder definitions may
        // override library names without changing constant availability.
        let mut runtime_values = BTreeMap::new();
        if target != EnvironmentTarget::JavaScript {
            for (name, library) in &sdk.libraries {
                if library.supported && !name.starts_with('_') {
                    runtime_values.insert(format!("dart.library.{name}"), "true".into());
                }
            }
            if product {
                runtime_values.insert("dart.library.mirrors".into(), "false".into());
            }
            runtime_values.insert("dart.vm.product".into(), product.to_string());
            runtime_values.insert("dart.isVM".into(), "true".into());
            // SDK 3.6.2 runtime/vm/globals.h:98-103: product runtimes retain
            // timeline support on macOS, Android and Fuchsia, not Linux/Windows.
            let timeline = !product
                || cfg!(any(
                    target_os = "macos",
                    target_os = "android",
                    target_os = "fuchsia"
                ));
            runtime_values.insert("dart.developer.timeline".into(), timeline.to_string());
            if target == EnvironmentTarget::NativeJit {
                for definition in definitions {
                    if let Some((name, value)) = definition.split_once('=')
                        && !name.is_empty()
                    {
                        runtime_values.insert(name.into(), value.into());
                    }
                }
            }
            runtime_values.insert("dartforge.compiler".into(), "true".into());
        }
        Ok(Self {
            target,
            values,
            runtime_values,
        })
    }

    /// Backend whose immutable constant and runtime rules apply.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::NativeJit, &SdkLayout::default(), false, &[]).unwrap();
    /// assert_eq!(env.target(), EnvironmentTarget::NativeJit);
    /// ```
    pub fn target(&self) -> EnvironmentTarget {
        self.target
    }

    /// Reads ordinary native constructors; AOT omits user definitions here.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::NativeAot, &SdkLayout::default(), true, &["flag=yes".into()]).unwrap();
    /// assert_eq!(env.get("flag"), Some("yes"));
    /// assert_eq!(env.runtime_get("flag"), None);
    /// ```
    pub fn runtime_get(&self, name: &str) -> Option<&str> {
        self.runtime_values.get(name).map(String::as_str)
    }

    /// Enumerates the runtime lookup table in stable key order.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::JavaScript, &SdkLayout::default(), false, &[]).unwrap();
    /// assert_eq!(env.runtime_entries().count(), 0);
    /// ```
    pub fn runtime_entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.runtime_values
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Enumerates constant declarations without exposing mutable storage.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::JavaScript, &SdkLayout::default(), false, &["flag=yes".into()]).unwrap();
    /// assert_eq!(env.entries().collect::<Vec<_>>(), vec![("flag", "yes")]);
    /// ```
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Looks up a declared string, preserving empty strings and absent names.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::JavaScript,
    ///     &SdkLayout::default(), false, &["empty=".into()]).unwrap();
    /// assert_eq!(env.get("empty"), Some(""));
    /// assert_eq!(env.get("missing"), None);
    /// ```
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Parses exactly `true` or `false`; other strings retain the caller's default.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::JavaScript,
    ///     &SdkLayout::default(), false, &["flag=false".into()]).unwrap();
    /// assert_eq!(env.boolean("flag"), Some(false));
    /// assert_eq!(env.boolean("missing"), None);
    /// ```
    pub fn boolean(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    /// Parses the SDK's signed decimal and hexadecimal forms, including trimming.
    /// Positive hex may use all 64 bits; JS retains its unsigned numeric value.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::NativeJit,
    ///     &SdkLayout::default(), false, &["n= 0x10 ".into()]).unwrap();
    /// assert_eq!(env.integer("n").and_then(EnvironmentInteger::exact_integer), Some(16));
    /// ```
    pub fn integer(&self, name: &str) -> Option<EnvironmentInteger> {
        self.parse_integer(self.get(name)?)
    }

    /// Parses an integer from the ordinary native environment.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::NativeJit, &SdkLayout::default(), false, &["n=42".into()]).unwrap();
    /// assert_eq!(env.runtime_integer("n"), Some(EnvironmentInteger::Native(42)));
    /// ```
    pub fn runtime_integer(&self, name: &str) -> Option<EnvironmentInteger> {
        self.parse_integer(self.runtime_get(name)?)
    }

    fn parse_integer(&self, text: &str) -> Option<EnvironmentInteger> {
        let text = text.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        let negative = text.starts_with('-');
        let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
        let hexadecimal = digits
            .strip_prefix("0x")
            .or_else(|| digits.strip_prefix("0X"));
        if let Some(digits) = hexadecimal {
            if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            let value = u64::from_str_radix(digits, 16).ok()?;
            if negative && value > (1u64 << 63) {
                return None;
            }
            return Some(if self.target == EnvironmentTarget::JavaScript {
                EnvironmentInteger::JavaScript(if negative {
                    -(value as f64)
                } else {
                    value as f64
                })
            } else {
                EnvironmentInteger::Native(if negative {
                    (value as i64).wrapping_neg()
                } else {
                    value as i64
                })
            });
        }
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let value = text.parse::<i64>().ok()?;
        Some(if self.target == EnvironmentTarget::JavaScript {
            EnvironmentInteger::JavaScript(value as f64)
        } else {
            EnvironmentInteger::Native(value)
        })
    }

    /// Stable, unambiguous cache identity including the backend and every value.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::JavaScript,
    ///     &SdkLayout::default(), false, &["flag=true".into()]).unwrap();
    /// assert!(env.identity().contains("flag"));
    /// ```
    pub fn identity(&self) -> String {
        let target = match self.target {
            EnvironmentTarget::NativeJit => "native-jit",
            EnvironmentTarget::NativeAot => "native-aot",
            EnvironmentTarget::JavaScript => "javascript",
        };
        serde_json::to_string(&(
            "compilation-environment-v2",
            target,
            &self.values,
            &self.runtime_values,
        ))
        .expect("string declarations serialize")
    }

    /// Restores the immutable context recorded in compiler IR and cache manifests.
    ///
    /// # Errors
    /// Invalid JSON, an unknown identity format or an unknown backend.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let env = CompilationEnvironment::new(EnvironmentTarget::NativeJit, &SdkLayout::default(), false, &["flag=yes".into()]).unwrap();
    /// assert_eq!(CompilationEnvironment::from_identity(&env.identity()).unwrap(), env);
    /// ```
    pub fn from_identity(identity: &str) -> Result<Self, String> {
        let (version, target, values, runtime_values): (
            String,
            String,
            BTreeMap<String, String>,
            BTreeMap<String, String>,
        ) = serde_json::from_str(identity)
            .map_err(|error| format!("invalid compilation environment: {error}"))?;
        if version != "compilation-environment-v2" {
            return Err(format!(
                "unsupported compilation environment format: {version}"
            ));
        }
        let target = match target.as_str() {
            "native-jit" => EnvironmentTarget::NativeJit,
            "native-aot" => EnvironmentTarget::NativeAot,
            "javascript" => EnvironmentTarget::JavaScript,
            _ => return Err(format!("unknown compilation environment target: {target}")),
        };
        Ok(Self {
            target,
            values,
            runtime_values,
        })
    }

    /// Installs the same immutable context for loading, queries and SDK caching.
    ///
    /// ```
    /// # use dartforge_elements::{SdkLayout, environment::*};
    /// let mut sdk = SdkLayout::default();
    /// let env = CompilationEnvironment::new(EnvironmentTarget::JavaScript,
    ///     &sdk, false, &["flag=true".into()]).unwrap();
    /// env.install(&mut sdk);
    /// assert!(sdk.compilation_environment.is_some());
    /// ```
    pub fn install(&self, sdk: &mut SdkLayout) {
        // SDK targets expose library support to conditional imports, not user
        // declarations. The latter are available to constant constructors.
        sdk.conditional_import_variables = Some(
            self.values
                .iter()
                .filter(|(name, _)| name.starts_with("dart.library."))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<HashMap<_, _>>(),
        );
        sdk.compilation_environment = Some(Arc::new(self.clone()));
        for (name, library) in &mut sdk.libraries {
            library.supported = self.get(&format!("dart.library.{name}")) == Some("true");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_runtime_declarations_follow_measured_vm_profiles() {
        let mut sdk = SdkLayout::default();
        for name in ["io", "mirrors", "_internal"] {
            sdk.libraries.insert(
                name.into(),
                crate::sdk::SdkLibrary {
                    name: name.into(),
                    path: Default::default(),
                    patches: vec![],
                    supported: true,
                },
            );
        }
        let definitions = [
            "flag=value".into(),
            "n=0xffffffffffffffff".into(),
            "dart.library.io=false".into(),
            "dart.library.unknown=false".into(),
            "dart.vm.product=false".into(),
            "dart.isVM=false".into(),
        ];
        let jit =
            CompilationEnvironment::new(EnvironmentTarget::NativeJit, &sdk, false, &definitions)
                .unwrap();
        let aot =
            CompilationEnvironment::new(EnvironmentTarget::NativeAot, &sdk, true, &definitions)
                .unwrap();
        assert_eq!(jit.get("dart.library.io"), Some("true"));
        assert_eq!(jit.runtime_get("dart.library.io"), Some("false"));
        assert_eq!(jit.get("dart.library.unknown"), None);
        assert_eq!(jit.runtime_get("dart.library.unknown"), Some("false"));
        assert_eq!(
            jit.runtime_integer("n"),
            Some(EnvironmentInteger::Native(-1))
        );
        assert_eq!(jit.get("dart.developer.timeline"), Some("true"));
        assert_eq!(aot.get("dart.developer.timeline"), None);
        let timeline = if cfg!(any(
            target_os = "macos",
            target_os = "android",
            target_os = "fuchsia"
        )) {
            "true"
        } else {
            "false"
        };
        assert_eq!(aot.runtime_get("dart.developer.timeline"), Some(timeline));
        assert_eq!(aot.get("flag"), Some("value"));
        assert_eq!(aot.runtime_get("flag"), None);
        assert_eq!(aot.get("dart.library.mirrors"), None);
        assert_eq!(aot.runtime_get("dart.library.mirrors"), Some("false"));
        assert_eq!(aot.get("dart.vm.product"), Some("false"));
        assert_eq!(aot.runtime_get("dart.vm.product"), Some("true"));
        for environment in [jit, aot] {
            assert_eq!(environment.get("dart.library._internal"), None);
            assert_eq!(environment.runtime_get("dart.library._internal"), None);
            assert_eq!(
                CompilationEnvironment::from_identity(&environment.identity()).unwrap(),
                environment
            );
        }
    }

    #[test]
    fn identity_rejects_unknown_schemas_and_backends() {
        for identity in [
            "[]",
            "[\"compilation-environment-v1\",\"native-jit\",{},{}]",
            "[\"compilation-environment-v2\",\"unknown\",{},{}]",
        ] {
            assert!(CompilationEnvironment::from_identity(identity).is_err());
        }
    }

    #[test]
    fn parsing_and_identity_preserve_values_and_order_independence() {
        let sdk = SdkLayout::default();
        let first = CompilationEnvironment::new(
            EnvironmentTarget::JavaScript,
            &sdk,
            false,
            &["a=first".into(), "a=hello=world".into(), "empty=".into()],
        )
        .unwrap();
        let second = CompilationEnvironment::new(
            EnvironmentTarget::JavaScript,
            &sdk,
            false,
            &["empty=".into(), "a=hello=world".into()],
        )
        .unwrap();
        assert_eq!(first.identity(), second.identity());
        assert_eq!(first.get("a"), Some("hello=world"));
        assert_eq!(first.get("empty"), Some(""));
        assert_eq!(first.get("absent"), None);
        assert!(
            CompilationEnvironment::new(EnvironmentTarget::JavaScript, &sdk, false, &["a".into()])
                .is_err()
        );
        assert!(
            CompilationEnvironment::new(EnvironmentTarget::NativeAot, &sdk, false, &["=x".into()])
                .is_err()
        );
        assert!(
            CompilationEnvironment::new(
                EnvironmentTarget::NativeJit,
                &sdk,
                false,
                &["a".into(), "=x".into()]
            )
            .is_ok()
        );
    }

    #[test]
    fn installed_context_separates_import_variables_analyzer_and_sdk_cache() {
        let analyzer = SdkLayout::default();
        let mut first = analyzer.clone();
        let environment = CompilationEnvironment::new(
            EnvironmentTarget::JavaScript,
            &first,
            false,
            &["flag=true".into()],
        )
        .unwrap();
        environment.install(&mut first);
        assert_eq!(first.compilation_environment.as_deref(), Some(&environment));
        assert!(
            !first
                .conditional_import_variables
                .as_ref()
                .unwrap()
                .contains_key("flag")
        );
        assert_ne!(
            crate::SdkCache::hash(&analyzer, "dartdevc"),
            crate::SdkCache::hash(&first, "dartdevc")
        );
        let mut changed = analyzer.clone();
        CompilationEnvironment::new(
            EnvironmentTarget::JavaScript,
            &changed,
            false,
            &["flag=false".into()],
        )
        .unwrap()
        .install(&mut changed);
        assert_ne!(
            crate::SdkCache::hash(&first, "dartdevc"),
            crate::SdkCache::hash(&changed, "dartdevc")
        );
        environment.install(&mut changed);
        assert_eq!(
            crate::SdkCache::hash(&first, "dartdevc"),
            crate::SdkCache::hash(&changed, "dartdevc")
        );
        assert!(analyzer.compilation_environment.is_none());
    }

    #[test]
    fn command_line_definitions_keep_order_and_the_first_equals() {
        let mut language = crate::sdk::LanguageOptions::default();
        let mut args = [
            "-Dflag=first",
            "--define=flag=last=value",
            "--define",
            "empty=",
            "input.dart",
        ]
        .into_iter();
        while let Some(arg) = args.next() {
            if !language.read_option(arg, &mut args).unwrap() {
                assert_eq!(arg, "input.dart");
            }
        }
        assert_eq!(
            language.defines,
            ["flag=first", "flag=last=value", "empty="]
        );
        let env = CompilationEnvironment::new(
            EnvironmentTarget::JavaScript,
            &SdkLayout::default(),
            false,
            &language.defines,
        )
        .unwrap();
        assert_eq!(env.get("flag"), Some("last=value"));
        assert!(
            language
                .read_option("--define", &mut [].into_iter())
                .is_err()
        );
    }

    #[test]
    fn profile_overrides_and_library_reservations_follow_the_target() {
        let mut sdk = SdkLayout::default();
        sdk.libraries.insert(
            "mirrors".into(),
            crate::sdk::SdkLibrary {
                name: "mirrors".into(),
                path: Default::default(),
                patches: vec![],
                supported: true,
            },
        );
        let defs = [
            "dart.vm.product=false".into(),
            "dart.isVM=false".into(),
            "dart.library.missing=true".into(),
        ];
        let aot =
            CompilationEnvironment::new(EnvironmentTarget::NativeAot, &sdk, true, &defs).unwrap();
        let jit =
            CompilationEnvironment::new(EnvironmentTarget::NativeJit, &sdk, false, &defs).unwrap();
        assert_eq!(aot.get("dart.isVM"), Some("true"));
        assert_eq!(jit.get("dart.isVM"), Some("false"));
        assert_eq!(aot.get("dart.vm.product"), Some("false"));
        assert_eq!(aot.get("dart.vm.profile"), None);
        assert_eq!(aot.get("dart.library.mirrors"), None);
        assert_eq!(jit.get("dart.library.mirrors"), Some("true"));
        assert_eq!(jit.get("dart.library.missing"), None);
        assert_ne!(aot.identity(), jit.identity());
    }

    #[test]
    fn environment_integer_values_match_measured_sdk_limits() {
        let sdk = SdkLayout::default();
        let cases = [
            ("42", Some(42)),
            ("+42", Some(42)),
            ("-0x10", Some(-16)),
            ("0xffffffffffffffff", Some(-1)),
            ("-0xffffffffffffffff", None),
            ("9223372036854775808", None),
            (" 42 ", Some(42)),
            ("\u{feff}42\u{feff}", Some(42)),
            ("\u{180e}42\u{180e}", None),
            ("1_000", None),
            ("1e3", None),
        ];
        for (text, expected) in cases {
            let env = CompilationEnvironment::new(
                EnvironmentTarget::NativeJit,
                &sdk,
                false,
                &[format!("n={text}")],
            )
            .unwrap();
            assert_eq!(
                env.integer("n").and_then(EnvironmentInteger::exact_integer),
                expected,
                "{text}"
            );
        }
        let web = CompilationEnvironment::new(
            EnvironmentTarget::JavaScript,
            &sdk,
            false,
            &["n=0xffffffffffffffff".into()],
        )
        .unwrap();
        assert_eq!(
            web.integer("n"),
            Some(EnvironmentInteger::JavaScript(u64::MAX as f64))
        );
        assert_eq!(
            web.integer("n").and_then(EnvironmentInteger::exact_integer),
            None
        );
    }
}
