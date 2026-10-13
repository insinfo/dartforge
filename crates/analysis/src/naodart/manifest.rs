//! The analyzer's `ManifestValidator`
//! (`analyzer/lib/src/manifest/manifest_validator.dart` and
//! `manifest_values.dart`, read entirely in 6.11.0;
//! docs/ANALYZER-ESPECIFICACAO-INFRA.md, batch II.10): the Chrome OS checks
//! over `AndroidManifest.xml`, which only run with
//! `analyzer: optional-checks: chrome-os-manifest-checks`.
//!
//! The XML parser is the original's, transcribed: it only keeps the
//! `manifest`, `application`, `activity`, `uses-feature` and
//! `uses-permission` elements, with tag and attribute names in lower case.
//! Written without compiling or running (2026-10-05).

use super::codigos_g::manifesto as c;
use super::Report;
use dartforge_diagnostics::Span;

const RELEVANT: [&str; 5] = ["activity", "application", "manifest", "uses-feature", "uses-permission"];

const UNSUPPORTED_HARDWARE: [&str; 37] = [
    "android.hardware.camera",
    "android.hardware.camera.autofocus",
    "android.hardware.camera.capability.manual_post_processing",
    "android.hardware.camera.capability.manual_sensor",
    "android.hardware.camera.capability.raw",
    "android.hardware.camera.flash",
    "android.hardware.camera.level.full",
    "android.hardware.consumerir",
    "android.hardware.location.gps",
    "android.hardware.nfc",
    "android.hardware.nfc.hce",
    "android.hardware.sensor.barometer",
    "android.hardware.telephony",
    "android.hardware.telephony.cdma",
    "android.hardware.telephony.gsm",
    "android.hardware.type.automotive",
    "android.hardware.type.television",
    "android.hardware.usb.accessory",
    "android.hardware.usb.host",
    "android.hardware.sensor.accelerometer",
    "android.hardware.sensor.compass",
    "android.hardware.sensor.gyroscope",
    "android.hardware.sensor.light",
    "android.hardware.sensor.proximity",
    "android.hardware.sensor.stepcounter",
    "android.hardware.sensor.stepdetector",
    "android.software.app_widgets",
    "android.software.device_admin",
    "android.software.home_screen",
    "android.software.input_methods",
    "android.software.leanback",
    "android.software.live_wallpaper",
    "android.software.live_tv",
    "android.software.managed_users",
    "android.software.midi",
    "android.software.sip",
    "android.software.sip.voip",
];

const UNSUPPORTED_ORIENTATIONS: [&str; 8] =
    ["landscape", "portrait", "reverseLandscape", "reversePortrait", "sensorLandscape", "sensorPortrait", "userLandscape", "userPortrait"];

/// `getImpliedUnsupportedHardware`: the hardware the permission implies.
fn implied_hardware(permission: &str) -> Option<&'static str> {
    match permission {
        "android.permission.CAMERA" => Some("android.hardware.camera"),
        "android.permission.CALL_PHONE"
        | "android.permission.CALL_PRIVILEGED"
        | "android.permission.MODIFY_PHONE_STATE"
        | "android.permission.PROCESS_OUTGOING_CALLS"
        | "android.permission.READ_SMS"
        | "android.permission.RECEIVE_SMS"
        | "android.permission.RECEIVE_MMS"
        | "android.permission.RECEIVE_WAP_PUSH"
        | "android.permission.SEND_SMS"
        | "android.permission.WRITE_APN_SETTINGS"
        | "android.permission.WRITE_SMS" => Some("android.hardware.telephony"),
        _ => None,
    }
}

struct Attribute {
    name: String,
    value: String,
    /// From the name to the closing quote, exclusive.
    span: Span,
}

struct Element {
    name: String,
    attributes: Vec<Attribute>,
    children: Vec<Element>,
    span: Span,
}

impl Element {
    fn attribute(&self, name: &str) -> Option<&Attribute> {
        // A repeated name: the last one wins (the original's map).
        self.attributes.iter().rev().find(|a| a.name == name)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.attribute(name).map(|a| a.value.as_str())
    }
}

/// What `parseXmlTag` found.
enum Tag {
    /// An element; `None` when it is not one of the relevant ones (or is a
    /// comment or a declaration).
    Element(Option<Element>),
    TagEnd,
    End,
    Error,
}

/// How an attribute list ends.
enum TagEnd {
    Tag,
    EmptyElement,
}

struct Reader<'a> {
    b: &'a [u8],
    source: &'a str,
    pos: usize,
}

impl Reader<'_> {
    fn closing(&self) -> bool {
        self.b.get(self.pos) == Some(&b'>')
    }

    fn closing_in_two(&self) -> bool {
        self.pos + 1 < self.b.len() && (self.b[self.pos] == b'?' || self.b[self.pos] == b'/') && self.b[self.pos + 1] == b'>'
    }

    fn blank(&self) -> bool {
        self.b.get(self.pos).is_some_and(u8::is_ascii_whitespace)
    }

    /// `parseXmlTag`.
    fn tag(&mut self) -> Tag {
        while self.pos < self.b.len() && self.b[self.pos] != b'<' {
            self.pos += 1;
        }
        if self.pos >= self.b.len() {
            return Tag::End;
        }
        let p = self.pos;
        if p + 3 < self.b.len() && &self.b[p + 1..p + 4] == b"!--" {
            // Comment: up to `-->`.
            self.pos += 4;
            loop {
                if self.pos + 2 >= self.b.len() {
                    return Tag::Error;
                }
                if &self.b[self.pos..self.pos + 3] == b"-->" {
                    break;
                }
                self.pos += 1;
            }
            self.pos += 2;
            return Tag::Element(None);
        }
        if p + 1 < self.b.len() && self.b[p + 1] == b'!' {
            // Declaration: up to `>`.
            self.pos += 2;
            while !self.closing() {
                self.pos += 1;
                if self.pos >= self.b.len() {
                    return Tag::Error;
                }
            }
            return Tag::Element(None);
        }
        self.normal()
    }

    /// `_parseAttribute`.
    fn attributes(&mut self, relevant: bool) -> Option<(Vec<Attribute>, TagEnd)> {
        let mut out = Vec::new();
        let end_ = loop {
            if self.pos >= self.b.len() {
                return None;
            }
            if self.closing() {
                break TagEnd::Tag;
            } else if self.closing_in_two() {
                self.pos += 1;
                break TagEnd::EmptyElement;
            } else if !self.blank() {
                return None;
            }
            while self.blank() {
                self.pos += 1;
            }
            if self.pos >= self.b.len() {
                return None;
            }
            if self.closing() {
                break TagEnd::Tag;
            } else if self.closing_in_two() {
                self.pos += 1;
                break TagEnd::EmptyElement;
            }
            // The name, up to the `=`.
            let name_start = self.pos;
            self.pos += 1;
            loop {
                if self.pos >= self.b.len() {
                    return None;
                }
                if self.b[self.pos] == b'=' {
                    break;
                }
                // An attribute without a value does not exist in XML.
                if self.blank() || self.closing() || self.closing_in_two() {
                    return None;
                }
                self.pos += 1;
            }
            let name = self.source.get(name_start..self.pos)?.to_lowercase();
            self.pos += 1;
            // The value, between quotes.
            let quote = *self.b.get(self.pos)?;
            if quote != b'\'' && quote != b'"' {
                return None;
            }
            self.pos += 1;
            let value_start = self.pos;
            while *self.b.get(self.pos)? != quote {
                self.pos += 1;
            }
            if relevant {
                let value = self.source.get(value_start..self.pos)?.to_string();
                out.push(Attribute { name, value, span: Span { start: name_start, end: self.pos } });
            }
            self.pos += 1;
        };
        Some((out, end_))
    }

    /// `_parseNormalTag`.
    fn normal(&mut self) -> Tag {
        let start = self.pos;
        self.pos += 1;
        if self.pos >= self.b.len() || self.blank() {
            return Tag::Error;
        }
        let closing = self.b[self.pos] == b'/';
        if closing {
            self.pos += 1;
        }
        let name_start = self.pos;
        while !self.closing() && !self.closing_in_two() && !self.blank() {
            self.pos += 1;
            if self.pos >= self.b.len() {
                return Tag::Error;
            }
        }
        let Some(name) = self.source.get(name_start..self.pos).map(str::to_lowercase) else { return Tag::Error };
        // `None`: not closed yet; `Some(empty)`: closed the tag.
        let mut closed: Option<bool> = None;
        if self.closing() {
            closed = Some(false);
        } else if self.closing_in_two() {
            closed = Some(true);
            self.pos += 1;
        }
        if closing {
            while self.blank() {
                self.pos += 1;
            }
            // A closing tag has no attributes.
            return if self.closing() { Tag::TagEnd } else { Tag::Error };
        }
        let relevant = RELEVANT.contains(&name.as_str());
        let (attributes, mut empty) = match closed {
            Some(empty) => (Vec::new(), empty),
            None => match self.attributes(relevant) {
                Some((attributes, end_)) => (attributes, matches!(end_, TagEnd::EmptyElement)),
                None => return Tag::Error,
            },
        };
        if name.starts_with('!') {
            empty = true;
        }
        let mut children = Vec::new();
        if !empty {
            self.pos += 1;
            loop {
                match self.tag() {
                    Tag::TagEnd => break,
                    Tag::End => return Tag::End,
                    Tag::Error => return Tag::Error,
                    Tag::Element(None) => {}
                    Tag::Element(Some(child)) => {
                        children.push(child);
                        self.pos += 1;
                    }
                }
            }
        }
        if relevant {
            let end = (self.pos + 1).min(self.b.len());
            Tag::Element(Some(Element { name, attributes, children, span: Span { start: start, end: end } }))
        } else {
            Tag::Element(None)
        }
    }
}

/// `_reportErrorForNode`: on attribute `key`, or on the whole element.
fn report(out: &mut Vec<Report>, element: &Element, key: Option<&str>, code: &'static super::CodigoNaoDart, args: &[&str]) {
    let span = key.and_then(|k| element.attribute(k)).map_or(element.span, |a| a.span);
    out.push(Report::new(code, span, args));
}

/// `ManifestValidator.validate` with `checkManifest` on: the reports of the
/// `AndroidManifest.xml` with text `source`.
pub fn validate(source: &str) -> Vec<Report> {
    let mut out = Vec::new();
    let mut reader = Reader { b: source.as_bytes(), source, pos: 0 };
    // Up to the `manifest` element.
    let manifesto = loop {
        match reader.tag() {
            Tag::End | Tag::Error => return out,
            Tag::Element(Some(e)) if e.name == "manifest" => break e,
            _ => {}
        }
    };
    let name = "android:name";
    let required = "android:required";
    let features: Vec<&Element> = manifesto.children.iter().filter(|e| e.name == "uses-feature").collect();
    let permissoes: Vec<&Element> = manifesto.children.iter().filter(|e| e.name == "uses-permission").collect();
    // `_validateTouchScreenFeature`.
    let tela = "android.hardware.touchscreen";
    match features.iter().find(|e| e.value(name) == Some(tela)) {
        Some(feature) => {
            if feature.attribute(required).is_none() {
                report(&mut out, feature, Some(name), &c::UNSUPPORTED_CHROME_OS_HARDWARE, &[tela]);
            } else if feature.value(required) == Some("true") {
                report(&mut out, feature, Some(name), &c::UNSUPPORTED_CHROME_OS_FEATURE, &[tela]);
            }
        }
        None => report(&mut out, &manifesto, None, &c::NO_TOUCHSCREEN_FEATURE, &[]),
    }
    // `_validateFeatures`.
    for feature in &features {
        let Some(value) = feature.value(name).filter(|v| UNSUPPORTED_HARDWARE.contains(v)) else { continue };
        if feature.attribute(required).is_none() {
            report(&mut out, feature, Some(name), &c::UNSUPPORTED_CHROME_OS_HARDWARE, &[value]);
        } else if feature.value(required) == Some("true") {
            report(&mut out, feature, Some(name), &c::UNSUPPORTED_CHROME_OS_FEATURE, &[value]);
        }
    }
    // `_validatePermissions`.
    let has = |feature: &str| features.iter().any(|e| e.value(name) == Some(feature));
    for permission in &permissoes {
        if permission.value(name) == Some("android.permission.CAMERA") {
            if !has("android.hardware.camera") || !has("android.hardware.camera.autofocus") {
                report(&mut out, permission, Some(name), &c::CAMERA_PERMISSIONS_INCOMPATIBLE, &[]);
            }
        } else if let Some(hardware) = permission.value(name).and_then(implied_hardware) {
            report(&mut out, permission, Some(name), &c::PERMISSION_IMPLIES_UNSUPPORTED_HARDWARE, &[hardware]);
        }
    }
    // `_validateActivity`, on the activities of the first `application`.
    if let Some(aplicacao) = manifesto.children.iter().find(|e| e.name == "application") {
        for activity in aplicacao.children.iter().filter(|e| e.name == "activity") {
            let orientation = "android:screenorientation";
            if activity.value(orientation).is_some_and(|v| UNSUPPORTED_ORIENTATIONS.contains(&v)) {
                report(&mut out, activity, Some(orientation), &c::SETTING_ORIENTATION_ON_ACTIVITY, &[]);
            }
            let redimensionavel = "android:resizeableactivity";
            if activity.value(redimensionavel) == Some("false") {
                report(&mut out, activity, Some(redimensionavel), &c::NON_RESIZABLE_ACTIVITY, &[]);
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn findings(source: &str) -> Vec<(&'static str, String)> {
        validate(source).into_iter().map(|r| (r.code.nome, source[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn sem_tela_de_toque() {
        let source = "<manifest>\n</manifest>\n";
        assert_eq!(findings(source), vec![("no_touchscreen_feature", "<manifest>\n</manifest>".to_string())]);
    }

    #[test]
    fn recursos_permissoes_e_atividades() {
        let source = concat!(
            "<?xml version=\"1.0\"?>\n",
            "<!-- comentário -->\n",
            "<manifest xmlns:android=\"x\">\n",
            "  <uses-feature android:name=\"android.hardware.touchscreen\" android:required=\"false\" />\n",
            "  <uses-feature android:name=\"android.hardware.nfc\" />\n",
            "  <uses-permission android:name=\"android.permission.CAMERA\" />\n",
            "  <uses-permission android:name=\"android.permission.SEND_SMS\" />\n",
            "  <application>\n",
            "    <activity android:screenOrientation=\"portrait\" android:resizeableActivity=\"false\"></activity>\n",
            "  </application>\n",
            "</manifest>\n",
        );
        assert_eq!(
            findings(source),
            vec![
                ("unsupported_chrome_os_hardware", "android:name=\"android.hardware.nfc".to_string()),
                ("camera_permissions_incompatible", "android:name=\"android.permission.CAMERA".to_string()),
                ("permission_implies_unsupported_hardware", "android:name=\"android.permission.SEND_SMS".to_string()),
                ("setting_orientation_on_activity", "android:screenOrientation=\"portrait".to_string()),
                ("non_resizable_activity", "android:resizeableActivity=\"false".to_string()),
            ]
        );
    }

    #[test]
    fn xml_quebrado_nao_relata() {
        assert!(findings("<manifest attr>").is_empty());
        assert!(findings("sem marcas").is_empty());
    }
}
