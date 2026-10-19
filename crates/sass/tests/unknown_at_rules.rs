//! Modern unknown at-rule values use the declaration-value grammar, while
//! the 1.66 compatibility mode keeps its historical parser.
use dartforge_sass::{from_string, DartSassVersion, InputSyntax, Options, OutputStyle};

#[test]
fn modern_at_rule_values_match_official_css() {
    for (syntax, source, expanded, compressed) in [
        (
            InputSyntax::Scss,
            include_str!("unknown_at_rules.scss"),
            include_str!("unknown_at_rules.expanded.css"),
            include_str!("unknown_at_rules.compressed.css"),
        ),
        (
            InputSyntax::Sass,
            include_str!("unknown_at_rules.sass"),
            include_str!("unknown_at_rules.sass.expanded.css"),
            include_str!("unknown_at_rules.sass.compressed.css"),
        ),
    ] {
        for version in [DartSassVersion::V1_101, DartSassVersion::V1_102_0] {
            for (style, expected) in [
                (OutputStyle::Expanded, expanded),
                (OutputStyle::Compressed, compressed),
            ] {
                let options = Options::default()
                    .quiet(true)
                    .version(version)
                    .style(style)
                    .input_syntax(syntax);
                let actual = from_string(source.to_string(), &options).unwrap();
                // The string API appends a final newline in expanded mode.
                let expected = if style == OutputStyle::Expanded {
                    format!("{expected}\n")
                } else {
                    expected.to_string()
                };
                assert_eq!(actual, expected, "{syntax:?}/{version:?}/{style:?}");
            }
        }
    }
}

#[test]
fn plain_css_still_rejects_silent_comments() {
    let options = Options::default()
        .quiet(true)
        .input_syntax(InputSyntax::Css);
    let source = "@foo bar // silent comment\n baz;";
    assert!(from_string(source.to_string(), &options).is_err());
}

#[test]
fn indented_declaration_newline_boundary_follows_the_selected_version() {
    let source = ".a\n  --value: (\n    content)\n";
    for version in [
        DartSassVersion::V1_66_0,
        DartSassVersion::V1_101,
        DartSassVersion::V1_102_0,
    ] {
        let options = Options::default()
            .quiet(true)
            .version(version)
            .input_syntax(InputSyntax::Sass);
        let result = from_string(source.to_string(), &options);
        assert_eq!(
            result.is_ok(),
            version != DartSassVersion::V1_66_0,
            "{version:?}: {result:?}"
        );
    }
}

#[test]
fn modern_at_rule_values_reject_unclosed_brackets_before_a_body() {
    for version in [DartSassVersion::V1_101, DartSassVersion::V1_102_0] {
        let options = Options::default().quiet(true).version(version);
        for source in ["@foo (nested {x: y});", "@foo [nested {x: y}];"] {
            assert!(
                from_string(source.to_string(), &options).is_err(),
                "{source}"
            );
        }
    }
}
