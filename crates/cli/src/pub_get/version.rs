//! Versions and version sets with the semantics of `package:pub_semver`
//! 2.1 (the one Pub 3.6.2 uses).
//!
//! A [`VersionSet`] is kept as sorted, disjoint, non-adjacent ranges, so set
//! operations are exact and equality is semantic. `pub_semver` keeps a few
//! representational distinctions (a `^` range, a single version) that only
//! matter for printing; [`VersionSet`]'s `Display` reproduces the terse
//! form Pub prints for solver terms.
use std::{cmp::Ordering, fmt};

/// One dot-separated identifier of a prerelease or build suffix: numeric
/// parts compare as numbers and sort before textual ones.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Part {
    Number(u64),
    Text(String),
}

impl Part {
    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a.cmp(b),
            (Self::Number(_), Self::Text(_)) => Ordering::Less,
            (Self::Text(_), Self::Number(_)) => Ordering::Greater,
            (Self::Text(a), Self::Text(b)) => a.cmp(b),
        }
    }
}

/// A `pub_semver` version: build suffixes take part in ordering and
/// equality, numeric identifiers are compared as numbers (`01` equals `1`),
/// and the original text is what `Display` prints.
#[derive(Clone, Debug)]
pub(crate) struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pre: Vec<Part>,
    build: Vec<Part>,
    text: String,
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.pre == other.pre
            && self.build == other.build
    }
}
impl Eq for Version {}

impl std::hash::Hash for Version {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.major, self.minor, self.patch, &self.pre, &self.build).hash(state);
    }
}

fn compare_parts(a: &[Part], b: &[Part]) -> Ordering {
    for index in 0..a.len().max(b.len()) {
        match (a.get(index), b.get(index)) {
            (Some(x), Some(y)) => {
                let order = x.compare(y);
                if order != Ordering::Equal {
                    return order;
                }
            }
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (None, None) => {}
        }
    }
    Ordering::Equal
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                _ => compare_parts(&self.pre, &other.pre),
            })
            .then_with(|| match (self.build.is_empty(), other.build.is_empty()) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => compare_parts(&self.build, &other.build),
            })
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// The length of the longest prefix of `text` that `pub_semver`'s
/// `startVersion` pattern matches, or `None`.
fn version_prefix(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    for part in 0..3 {
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == start {
            return None;
        }
        if part < 2 {
            if bytes.get(index) != Some(&b'.') {
                return None;
            }
            index += 1;
        }
    }
    // `(-ids)?(\+ids)?`, where ids are dot-separated `[0-9A-Za-z-]+`.
    let identifiers = |mut index: usize| -> usize {
        let mut end = index;
        loop {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'-')
            {
                index += 1;
            }
            if index == start {
                return end;
            }
            end = index;
            if bytes.get(index) != Some(&b'.') {
                return end;
            }
            index += 1;
        }
    };
    for marker in *b"-+" {
        if bytes.get(index) == Some(&marker) {
            let end = identifiers(index + 1);
            if end > index + 1 {
                index = end;
            }
        }
    }
    Some(index)
}

fn split_parts(text: &str) -> Vec<Part> {
    text.split('.')
        .map(|part| match part.parse::<u64>() {
            Ok(number) if part.bytes().all(|b| b.is_ascii_digit()) => Part::Number(number),
            _ => Part::Text(part.to_owned()),
        })
        .collect()
}

impl Version {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: Vec::new(),
            build: Vec::new(),
            text: format!("{major}.{minor}.{patch}"),
        }
    }

    /// `Version.parse`: the whole text must be a version.
    pub fn parse(text: &str) -> Result<Self, String> {
        if version_prefix(text) != Some(text.len()) {
            return Err(format!("Could not parse \"{text}\"."));
        }
        let (core, build) = match text.split_once('+') {
            Some((core, build)) => (core, Some(build)),
            None => (text, None),
        };
        let (numbers, pre) = match core.split_once('-') {
            Some((numbers, pre)) => (numbers, Some(pre)),
            None => (core, None),
        };
        let mut numbers = numbers.split('.').map(|n| {
            n.parse::<u64>()
                .map_err(|_| format!("Could not parse \"{text}\"."))
        });
        let (Some(major), Some(minor), Some(patch)) =
            (numbers.next(), numbers.next(), numbers.next())
        else {
            return Err(format!("Could not parse \"{text}\"."));
        };
        Ok(Self {
            major: major?,
            minor: minor?,
            patch: patch?,
            pre: pre.map(split_parts).unwrap_or_default(),
            build: build.map(split_parts).unwrap_or_default(),
            text: text.to_owned(),
        })
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }

    pub fn has_build(&self) -> bool {
        !self.build.is_empty()
    }

    /// `x.y.z-0`, the lowest prerelease of this release.
    pub fn first_prerelease(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor,
            patch: self.patch,
            pre: vec![Part::Number(0)],
            build: Vec::new(),
            text: format!("{}.{}.{}-0", self.major, self.minor, self.patch),
        }
    }

    pub fn is_first_prerelease(&self) -> bool {
        self.pre == [Part::Number(0)]
    }

    /// The next version that `^` excludes: the next minor below 1.0.0.
    pub fn next_breaking(&self) -> Self {
        if self.major == 0 {
            Self::new(0, self.minor + 1, 0)
        } else {
            Self::new(self.major + 1, 0, 0)
        }
    }

    fn same_release(&self, other: &Self) -> bool {
        (self.major, self.minor, self.patch) == (other.major, other.minor, other.patch)
    }
}

/// An interval of versions; `None` bounds are unbounded.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Range {
    pub min: Option<Version>,
    pub max: Option<Version>,
    pub include_min: bool,
    pub include_max: bool,
}

impl Range {
    fn any() -> Self {
        Self {
            min: None,
            max: None,
            include_min: false,
            include_max: false,
        }
    }

    fn single(version: Version) -> Self {
        Self {
            min: Some(version.clone()),
            max: Some(version),
            include_min: true,
            include_max: true,
        }
    }

    /// `VersionRange(...)` without `alwaysIncludeMaxPreRelease`: an
    /// exclusive stable maximum also excludes its prereleases, unless the
    /// minimum is a prerelease of the same release.
    pub fn user(
        min: Option<Version>,
        include_min: bool,
        max: Option<Version>,
        include_max: bool,
    ) -> Self {
        let max = match max {
            Some(max)
                if !include_max
                    && !max.is_prerelease()
                    && !max.has_build()
                    && min
                        .as_ref()
                        .is_none_or(|min| !min.is_prerelease() || !min.same_release(&max)) =>
            {
                Some(max.first_prerelease())
            }
            max => max,
        };
        Self {
            min,
            max,
            include_min,
            include_max,
        }
    }

    pub fn allows(&self, version: &Version) -> bool {
        if let Some(min) = &self.min {
            match version.cmp(min) {
                Ordering::Less => return false,
                Ordering::Equal if !self.include_min => return false,
                _ => {}
            }
        }
        if let Some(max) = &self.max {
            match version.cmp(max) {
                Ordering::Greater => return false,
                Ordering::Equal if !self.include_max => return false,
                _ => {}
            }
        }
        true
    }

    fn is_empty(&self) -> bool {
        match (&self.min, &self.max) {
            (Some(min), Some(max)) => match min.cmp(max) {
                Ordering::Greater => true,
                Ordering::Equal => !(self.include_min && self.include_max),
                Ordering::Less => false,
            },
            _ => false,
        }
    }

    /// Orders ranges by their lower bound (an inclusive bound first), then
    /// by their upper bound, like `VersionRange.compareTo`.
    fn compare(&self, other: &Self) -> Ordering {
        let lower = match (&self.min, &other.min) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => a
                .cmp(b)
                .then_with(|| other.include_min.cmp(&self.include_min)),
        };
        lower.then_with(|| match (&self.max, &other.max) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(a), Some(b)) => a
                .cmp(b)
                .then_with(|| self.include_max.cmp(&other.include_max)),
        })
    }

    fn intersect(&self, other: &Self) -> Option<Self> {
        let (min, include_min) = match (&self.min, &other.min) {
            (None, _) => (other.min.clone(), other.include_min),
            (_, None) => (self.min.clone(), self.include_min),
            (Some(a), Some(b)) => match a.cmp(b) {
                Ordering::Greater => (self.min.clone(), self.include_min),
                Ordering::Less => (other.min.clone(), other.include_min),
                Ordering::Equal => (self.min.clone(), self.include_min && other.include_min),
            },
        };
        let (max, include_max) = match (&self.max, &other.max) {
            (None, _) => (other.max.clone(), other.include_max),
            (_, None) => (self.max.clone(), self.include_max),
            (Some(a), Some(b)) => match a.cmp(b) {
                Ordering::Less => (self.max.clone(), self.include_max),
                Ordering::Greater => (other.max.clone(), other.include_max),
                Ordering::Equal => (self.max.clone(), self.include_max && other.include_max),
            },
        };
        let range = Self {
            min,
            max,
            include_min,
            include_max,
        };
        (!range.is_empty()).then_some(range)
    }

    /// Whether `next`, which does not sort before `self`, overlaps or
    /// touches it, so that their union is a single range.
    fn joins(&self, next: &Self) -> bool {
        match (&self.max, &next.min) {
            (None, _) | (_, None) => true,
            (Some(max), Some(min)) => match max.cmp(min) {
                Ordering::Greater => true,
                Ordering::Equal => self.include_max || next.include_min,
                Ordering::Less => false,
            },
        }
    }
}

/// A set of versions (`VersionConstraint`): `any`, empty, a version, a
/// range or a union of ranges.
#[derive(Clone, Debug)]
pub(crate) struct VersionSet {
    ranges: Vec<Range>,
    /// Written as `^x.y.z` (`CompatibleWithVersionRange`); only printing
    /// depends on it.
    caret: bool,
}

impl PartialEq for VersionSet {
    fn eq(&self, other: &Self) -> bool {
        self.ranges == other.ranges
    }
}
impl Eq for VersionSet {}

impl std::hash::Hash for VersionSet {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ranges.hash(state);
    }
}

/// [`VersionSet`] printed as Pub prints solver terms: `^x.y.z` whenever
/// the range has that shape (`withTerseConstraint`).
pub(crate) struct Terse<'a>(pub &'a VersionSet);

impl fmt::Display for Terse<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(min) = self.0.caret() {
            return write!(f, "^{min}");
        }
        fmt::Display::fmt(self.0, f)
    }
}

impl VersionSet {
    pub fn any() -> Self {
        Self {
            ranges: vec![Range::any()],
            caret: false,
        }
    }

    pub fn empty() -> Self {
        Self {
            ranges: Vec::new(),
            caret: false,
        }
    }

    pub fn exact(version: &Version) -> Self {
        Self {
            ranges: vec![Range::single(version.clone())],
            caret: false,
        }
    }

    pub fn from_range(range: Range) -> Self {
        Self::from_ranges(vec![range])
    }

    /// `^version`.
    pub fn compatible_with(version: &Version) -> Self {
        let mut set = Self::from_range(Range {
            min: Some(version.clone()),
            max: Some(version.next_breaking().first_prerelease()),
            include_min: true,
            include_max: false,
        });
        set.caret = true;
        set
    }

    /// `asCompatibleWithIfPossible`: printed with `^` when it has that
    /// shape.
    #[allow(clippy::wrong_self_convention)]
    pub fn as_compatible_with_if_possible(mut self) -> Self {
        self.caret = self.caret().is_some();
        self
    }

    /// The terse rendering of solver terms.
    pub fn terse(&self) -> Terse<'_> {
        Terse(self)
    }

    fn from_ranges(mut ranges: Vec<Range>) -> Self {
        ranges.retain(|r| !r.is_empty());
        // An unbounded side has no inclusivity; keep equality semantic.
        for range in &mut ranges {
            range.include_min &= range.min.is_some();
            range.include_max &= range.max.is_some();
        }
        ranges.sort_by(Range::compare);
        let mut merged: Vec<Range> = Vec::with_capacity(ranges.len());
        for range in ranges {
            match merged.last_mut() {
                Some(last) if last.joins(&range) => {
                    let higher = match (&last.max, &range.max) {
                        (None, _) => false,
                        (_, None) => true,
                        (Some(a), Some(b)) => match a.cmp(b) {
                            Ordering::Less => true,
                            Ordering::Equal => range.include_max && !last.include_max,
                            Ordering::Greater => false,
                        },
                    };
                    if higher {
                        last.max = range.max;
                        last.include_max = range.include_max;
                    }
                }
                _ => merged.push(range),
            }
        }
        Self {
            ranges: merged,
            caret: false,
        }
    }

    pub fn ranges(&self) -> &[Range] {
        &self.ranges
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn is_any(&self) -> bool {
        self.ranges.len() == 1 && self.ranges[0].min.is_none() && self.ranges[0].max.is_none()
    }

    /// The single version this set contains, if it is one.
    pub fn as_version(&self) -> Option<&Version> {
        match self.ranges.as_slice() {
            [range] if range.include_min && range.include_max && range.min == range.max => {
                range.min.as_ref()
            }
            _ => None,
        }
    }

    pub fn allows(&self, version: &Version) -> bool {
        self.ranges.iter().any(|r| r.allows(version))
    }

    pub fn intersect(&self, other: &Self) -> Self {
        let mut result = Vec::new();
        for a in &self.ranges {
            for b in &other.ranges {
                if let Some(range) = a.intersect(b) {
                    result.push(range);
                }
            }
        }
        Self::from_ranges(result)
    }

    pub fn union(&self, other: &Self) -> Self {
        Self::from_ranges(self.ranges.iter().chain(&other.ranges).cloned().collect())
    }

    pub fn complement(&self) -> Self {
        let mut result = Vec::new();
        let mut previous: Option<(Version, bool)> = None;
        let mut open_start = true;
        for range in &self.ranges {
            if let Some(min) = &range.min {
                let (gap_min, gap_include_min) = match &previous {
                    Some((v, inclusive)) => (Some(v.clone()), !inclusive),
                    None => (None, false),
                };
                if open_start || gap_min.is_some() {
                    result.push(Range {
                        min: gap_min,
                        include_min: gap_include_min,
                        max: Some(min.clone()),
                        include_max: !range.include_min,
                    });
                }
            }
            open_start = false;
            match &range.max {
                Some(max) => previous = Some((max.clone(), range.include_max)),
                None => return Self::from_ranges(result),
            }
        }
        match previous {
            Some((max, inclusive)) => result.push(Range {
                min: Some(max),
                include_min: !inclusive,
                max: None,
                include_max: false,
            }),
            None => result.push(Range::any()),
        }
        Self::from_ranges(result)
    }

    pub fn difference(&self, other: &Self) -> Self {
        self.intersect(&other.complement())
    }

    pub fn allows_all(&self, other: &Self) -> bool {
        other.difference(self).is_empty()
    }

    pub fn allows_any(&self, other: &Self) -> bool {
        !self.intersect(other).is_empty()
    }

    /// `VersionConstraint.parse`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let original = text;
        let mut text = text.trim();
        if text == "any" {
            return Ok(Self::any());
        }
        let take_version = |text: &mut &str| -> Result<Option<Version>, String> {
            let Some(length) = version_prefix(text) else {
                return Ok(None);
            };
            let version = Version::parse(&text[..length])?;
            *text = &text[length..];
            Ok(Some(version))
        };
        if let Some(rest) = text.strip_prefix('^') {
            text = rest.trim();
            let Some(version) = take_version(&mut text)? else {
                return Err(format!(
                    "Expected version number after \"^\" in \"{original}\", got \"{text}\"."
                ));
            };
            if !text.is_empty() {
                return Err(format!(
                    "Cannot include other constraints with \"^\" constraint in \"{original}\"."
                ));
            }
            return Ok(Self::compatible_with(&version));
        }
        let mut min: Option<Version> = None;
        let mut include_min = false;
        let mut max: Option<Version> = None;
        let mut include_max = false;
        loop {
            text = text.trim();
            if text.is_empty() {
                break;
            }
            let (new_min, new_include_min, new_max, new_include_max) = if let Some(version) =
                take_version(&mut text)?
            {
                (Some(version.clone()), true, Some(version), true)
            } else {
                let operator = ["<=", ">=", "<", ">"]
                    .into_iter()
                    .find(|op| text.starts_with(op))
                    .ok_or_else(|| {
                        format!(
                            "Could not parse version \"{original}\". Unknown text at \"{text}\"."
                        )
                    })?;
                text = text[operator.len()..].trim();
                let Some(version) = take_version(&mut text)? else {
                    return Err(format!(
                        "Expected version number after \"{operator}\" in \"{original}\", got \"{text}\"."
                    ));
                };
                match operator {
                    "<=" => (None, false, Some(version), true),
                    "<" => (None, false, Some(version), false),
                    ">=" => (Some(version), true, None, false),
                    _ => (Some(version), false, None, false),
                }
            };
            if let Some(new_min) = new_min {
                match &min {
                    Some(current) if new_min < *current => {}
                    Some(current) if new_min == *current => {
                        if !new_include_min {
                            include_min = false;
                        }
                    }
                    _ => {
                        min = Some(new_min);
                        include_min = new_include_min;
                    }
                }
            }
            if let Some(new_max) = new_max {
                match &max {
                    Some(current) if new_max > *current => {}
                    Some(current) if new_max == *current => {
                        if !new_include_max {
                            include_max = false;
                        }
                    }
                    _ => {
                        max = Some(new_max);
                        include_max = new_include_max;
                    }
                }
            }
        }
        if min.is_none() && max.is_none() {
            return Err("Cannot parse an empty string.".into());
        }
        if let (Some(low), Some(high)) = (&min, &max) {
            match low.cmp(high) {
                Ordering::Greater => return Ok(Self::empty()),
                Ordering::Equal => {
                    return Ok(if include_min && include_max {
                        Self::exact(low)
                    } else {
                        Self::empty()
                    });
                }
                Ordering::Less => {}
            }
        }
        Ok(Self::from_range(Range::user(
            min,
            include_min,
            max,
            include_max,
        )))
    }

    /// The `^` form when this set is exactly `^min`.
    fn caret(&self) -> Option<&Version> {
        match self.ranges.as_slice() {
            [range] if range.include_min && !range.include_max => {
                let min = range.min.as_ref()?;
                (range.max.as_ref() == Some(&min.next_breaking().first_prerelease())).then_some(min)
            }
            _ => None,
        }
    }
}

fn write_range(f: &mut fmt::Formatter<'_>, range: &Range) -> fmt::Result {
    if let Some(min) = &range.min
        && range.min == range.max
        && range.include_min
        && range.include_max
    {
        return write!(f, "{min}");
    }
    if let Some(min) = &range.min {
        write!(f, "{}{min}", if range.include_min { ">=" } else { ">" })?;
    }
    if let Some(max) = &range.max {
        if range.min.is_some() {
            f.write_str(" ")?;
        }
        if range.include_max {
            write!(f, "<={max}")?;
        } else if max.is_first_prerelease() {
            write!(f, "<{}.{}.{}", max.major, max.minor, max.patch)?;
        } else {
            write!(f, "<{max}")?;
            let min_is_prerelease_of_max = range
                .min
                .as_ref()
                .is_some_and(|min| min.is_prerelease() && min.same_release(max));
            if !max.is_prerelease() && !max.has_build() && !min_is_prerelease_of_max {
                f.write_str("-∞")?;
            }
        }
    }
    if range.min.is_none() && range.max.is_none() {
        f.write_str("any")?;
    }
    Ok(())
}

impl fmt::Display for VersionSet {
    /// `VersionConstraint.toString`: `^x.y.z` for a range written that
    /// way, `<empty>` for the empty set, and ranges joined by ` or `.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ranges.is_empty() {
            return f.write_str("<empty>");
        }
        if self.caret
            && let Some(min) = self.caret()
        {
            return write!(f, "^{min}");
        }
        for (index, range) in self.ranges.iter().enumerate() {
            if index > 0 {
                f.write_str(" or ")?;
            }
            write_range(f, range)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }
    fn c(text: &str) -> VersionSet {
        VersionSet::parse(text).unwrap()
    }

    #[test]
    fn versions_order_like_pub_semver() {
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.0+1",
            "1.0.0+2",
            "1.0.0+build",
            "1.0.1",
        ];
        for pair in ordered.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
        assert_eq!(v("1.0.0-01"), v("1.0.0-1"));
        assert_eq!(v("1.0.0-01").to_string(), "1.0.0-01");
        assert!(Version::parse("1.0").is_err());
        assert!(Version::parse("1.0.0-").is_err());
    }

    #[test]
    fn constraints_parse_like_pub_semver() {
        let caret = c("^1.2.3");
        assert!(caret.allows(&v("1.9.0")));
        assert!(!caret.allows(&v("2.0.0-dev")));
        assert_eq!(caret, c(">=1.2.3 <2.0.0"));
        assert_eq!(caret.to_string(), "^1.2.3");
        assert_eq!(c(">=1.2.3 <2.0.0").to_string(), ">=1.2.3 <2.0.0");
        assert_eq!(c(">=1.2.3 <2.0.0").terse().to_string(), "^1.2.3");
        assert_eq!(
            caret.intersect(&VersionSet::any()).to_string(),
            ">=1.2.3 <2.0.0"
        );
        assert_eq!(c("^0.2.3"), c(">=0.2.3 <0.3.0"));
        // An exclusive stable upper bound excludes its prereleases...
        assert!(!c("<2.0.0").allows(&v("2.0.0-dev")));
        // ...unless the lower bound is a prerelease of the same release.
        assert!(c(">=2.0.0-dev <2.0.0").allows(&v("2.0.0-dev.2")));
        assert!(c(">=2.0.0-dev<3.0.0").allows(&v("2.5.0")));
        assert_eq!(c(">=1.0.0 <=1.0.0"), VersionSet::exact(&v("1.0.0")));
        assert!(c(">2.0.0 <1.0.0").is_empty());
        assert!(c("any").is_any());
        assert!(VersionSet::parse("=1.0.0").is_err());
        assert!(VersionSet::parse("").is_err());
        assert_eq!(c(">=3.4.0 <4.0.0").to_string(), ">=3.4.0 <4.0.0");
        assert_eq!(c(">=1.0.0 <1.5.0").to_string(), ">=1.0.0 <1.5.0");
    }

    #[test]
    fn set_operations_are_exact() {
        let a = c(">=1.0.0 <3.0.0");
        let b = c(">=2.0.0 <4.0.0");
        assert_eq!(a.intersect(&b), c(">=2.0.0 <3.0.0"));
        assert_eq!(a.union(&b), c(">=1.0.0 <4.0.0"));
        let hole = a.difference(&VersionSet::exact(&v("2.0.0")));
        assert!(!hole.allows(&v("2.0.0")));
        assert!(hole.allows(&v("2.0.1")));
        assert_eq!(hole.ranges().len(), 2);
        // pub_semver marks an exclusive stable maximum that keeps its prereleases.
        assert_eq!(hole.to_string(), ">=1.0.0 <2.0.0-∞ or >2.0.0 <3.0.0");
        let everything_but = VersionSet::any().difference(&VersionSet::exact(&v("1.0.0")));
        assert_eq!(
            VersionSet::any().difference(&everything_but).as_version(),
            Some(&v("1.0.0"))
        );
        assert!(a.allows_all(&c("^1.5.0")));
        assert!(!a.allows_all(&b));
        assert!(a.allows_any(&b));
        assert!(VersionSet::empty().complement().is_any());
        assert!(VersionSet::any().complement().is_empty());
        assert_eq!(a.complement().complement(), a);
    }
}
