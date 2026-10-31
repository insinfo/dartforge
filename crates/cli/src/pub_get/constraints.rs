//! Dart-style version intervals, without npm's implicit prerelease exclusion.
use semver::Version;

#[derive(Clone, Debug)]
pub(super) struct Constraint {
    pub text: String,
    pub lower: Option<(Version, bool)>,
    pub upper: Option<(Version, bool)>,
}
impl Constraint {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut result = Self {
            text: text.into(),
            lower: None,
            upper: None,
        };
        let text = text.trim();
        if text.is_empty() || text == "any" {
            return Ok(result);
        }
        if let Some(value) = text.strip_prefix('^') {
            let version = parse_version(value.trim())?;
            let upper = if version.major > 0 {
                Version::new(
                    version.major.checked_add(1).ok_or("version overflow")?,
                    0,
                    0,
                )
            } else {
                Version::new(
                    0,
                    version.minor.checked_add(1).ok_or("version overflow")?,
                    0,
                )
            };
            result.lower = Some((version, true));
            result.upper = Some((upper, false));
            return Ok(result);
        }
        // Like pub_semver, an operator is followed by a greedily read version,
        // with or without spaces in between: `>=2.0.0-dev<3.0.0` is valid.
        let mut parts = Vec::new();
        let mut rest = text;
        loop {
            rest = rest.trim_start();
            if rest.is_empty() {
                break;
            }
            let operator = [">=", "<=", ">", "<", "="]
                .into_iter()
                .find(|op| rest.starts_with(op))
                .unwrap_or("");
            rest = rest[operator.len()..].trim_start();
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')))
                .unwrap_or(rest.len());
            if end == 0 {
                return Err(format!("missing version after {operator:?} in {text}"));
            }
            parts.push(format!("{operator}{}", &rest[..end]));
            rest = &rest[end..];
        }
        for part in &parts {
            let (operator, number) = [">=", "<=", ">", "<", "="]
                .into_iter()
                .find_map(|op| part.strip_prefix(op).map(|n| (op, n)))
                .unwrap_or(("=", part.as_str()));
            let version = parse_version(number)?;
            match operator {
                ">=" | ">" => {
                    if result.lower.is_some() {
                        return Err(format!("unsupported repeated lower bound: {text}"));
                    }
                    result.lower = Some((version, operator == ">="));
                }
                "<=" | "<" => {
                    if result.upper.is_some() {
                        return Err(format!("unsupported repeated upper bound: {text}"));
                    }
                    result.upper = Some((version, operator == "<="));
                }
                _ => {
                    if result.lower.is_some() || result.upper.is_some() {
                        return Err(format!("unsupported version constraint: {text}"));
                    }
                    result.lower = Some((version.clone(), true));
                    result.upper = Some((version, true));
                }
            }
        }
        Ok(result)
    }
    pub fn allows(&self, version: &Version) -> bool {
        let lower =
            self.lower
                .as_ref()
                .is_none_or(|(v, inclusive)| match compare_versions(version, v) {
                    std::cmp::Ordering::Greater => true,
                    std::cmp::Ordering::Equal => *inclusive,
                    _ => false,
                });
        let upper = self.upper.as_ref().is_none_or(|(v, inclusive)| {
            // Pub's exclusive stable upper bound excludes prereleases of that release.
            if !inclusive
                && v.pre.is_empty()
                && v.build.is_empty()
                && !version.pre.is_empty()
                && (v.major, v.minor, v.patch) == (version.major, version.minor, version.patch)
                && !self.lower.as_ref().is_some_and(|(lower, _)| {
                    !lower.pre.is_empty()
                        && (lower.major, lower.minor, lower.patch) == (v.major, v.minor, v.patch)
                })
            {
                return false;
            }
            match compare_versions(version, v) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Equal => *inclusive,
                _ => false,
            }
        });
        lower && upper
    }
    pub fn for_sdk(&self, sdk: &Version) -> Self {
        let mut result = self.clone();
        if sdk.major >= 3
            && self
                .lower
                .as_ref()
                .is_some_and(|(v, _)| (v.major, v.minor) >= (2, 12))
            && self.upper.as_ref().is_some_and(|(v, inclusive)| {
                !inclusive && compare_versions(v, &Version::new(3, 0, 0)).is_eq()
            })
        {
            result.upper = Some((Version::new(4, 0, 0), false));
        }
        result
    }
    pub fn language(&self, _sdk: &Version) -> String {
        self.lower
            .as_ref()
            .map(|(version, _)| format!("{}.{}", version.major, version.minor))
            .unwrap_or_else(|| "2.7".into())
    }
}
pub(super) fn parse_version(text: &str) -> Result<Version, String> {
    Version::parse(text).map_err(|e| format!("unsupported or invalid version {text:?}: {e}"))
}

/// Pub includes build suffixes in ordering, unlike SemVer 2.0 precedence.
pub(super) fn compare_versions(left: &Version, right: &Version) -> std::cmp::Ordering {
    left.cmp_precedence(right).then_with(|| {
        let left = left.build.as_str();
        let right = right.build.as_str();
        if left.is_empty() || right.is_empty() {
            return (!left.is_empty()).cmp(&!right.is_empty());
        }
        let mut a = left.split('.');
        let mut b = right.split('.');
        loop {
            let (Some(a), Some(b)) = (a.next(), b.next()) else {
                return left.split('.').count().cmp(&right.split('.').count());
            };
            let numeric_a = a.bytes().all(|x| x.is_ascii_digit());
            let numeric_b = b.bytes().all(|x| x.is_ascii_digit());
            let comparison = match (numeric_a, numeric_b) {
                (true, true) => {
                    let a = a.trim_start_matches('0');
                    let b = b.trim_start_matches('0');
                    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
                }
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                (false, false) => a.cmp(b),
            };
            if !comparison.is_eq() {
                return comparison;
            }
        }
    })
}
