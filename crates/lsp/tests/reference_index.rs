//! The resolved-reference index answers `references` as the search without it
//! (docs/LSP.md, "Décima quinta rodada"): at every identifier of a project
//! with the forms the search treats apart, before and after edits.
mod common;

use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{DocumentStore, SemanticAnalyzer};

const MODEL: &str = r#"/// A model; see [Model.named], [Model.new], [label] and [Base.describe].
class Model extends Base {
  final String label;
  int count;
  final int? extra;

  /// Builds with [label].
  Model(this.label, [this.count = 0, this.extra]) : super('model');
  Model.named({required this.label, this.count = 1, this.extra}) : super.named();
  factory Model.redirect(String label) = Model;
  Model.other() : this('other');

  @override
  String describe() => '$label $count';
}

abstract class Base {
  final String kind;
  Base(this.kind);
  Base.named() : kind = 'named';
  String describe();
}

class Child extends Model {
  Child(super.label, [super.count]);
  Child.named({required super.label}) : super.named();
  @override
  String describe() => 'child ${super.describe()}';
}

class Plain extends Base {
  Plain() : super('plain');
  @override
  String describe() => kind;
}

class Implicit extends Plain {}

enum Color { red, green.custom(2), blue;
  const Color() : value = 0;
  const Color.custom(this.value);
  final int value;
}

extension Describe on Base {
  String get shout => describe().toUpperCase();
  static String make() => 'x';
}

const marker = Marker.named();

class Marker {
  const Marker();
  const Marker.named();
}

int counter = 0;
int get total => counter;
set total(int v) => counter = v;
"#;

const USE: &str = r#"import 'model.dart' show Model, Child, Base, Color, counter, total, Describe, Marker;
import 'model.dart' as m hide Plain;

/// Uses [Model.describe] and [m.Child].
@Marker.named()
@m.Marker()
class User {
  Model model = Model('a', 2, 3);
  final m.Child child = m.Child.named(label: 'c');

  void run() {
    final other = Model.named(label: 'b', count: 3);
    final copy = Model.redirect('r');
    var x = Model.other();
    print(model.describe());
    print(child.describe());
    print(other.label);
    print(copy.count);
    print(x.extra);
    print(model.shout);
    print(Describe.make());
    print(Color.green.value);
    counter++;
    total = total + 1;
    final tear = Model.named;
    print(tear);
    print(m.counter);
    final Base b = Child('q');
    print(b.describe() + b.kind);
    print(Implicit().describe());
  }
}
"#;

fn analyzer(project: &common::Project) -> SemanticAnalyzer {
    let sdk = SdkLayout::load(&project.root.join("sdk/lib"), "dartdevc").unwrap();
    SemanticAnalyzer::new(Some(sdk))
}

fn uri_of(project: &common::Project, relative: &str) -> String {
    url::Url::from_file_path(project.root.join("projeto").join(relative))
        .unwrap()
        .to_string()
}

/// The check's report has no difference and no target left to the search.
fn assert_same(report: &str, at_least: usize) {
    let compared: usize = report
        .lines()
        .find_map(|l| l.strip_prefix("compared "))
        .and_then(|l| l.split(',').next())
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or_else(|| panic!("no comparison:\n{report}"));
    assert!(
        report.contains("different 0;"),
        "the index differs from the search:\n{report}"
    );
    assert!(
        !report.contains("not answered"),
        "targets left to the search:\n{report}"
    );
    assert!(compared >= at_least, "only {compared} targets:\n{report}");
}

#[test]
fn the_index_answers_as_the_search() {
    let project = common::Project::new("reference-index");
    project.write("lib/model.dart", MODEL);
    project.write("lib/use.dart", USE);
    let mut documents = DocumentStore::new();
    let uris = vec![
        uri_of(&project, "lib/model.dart"),
        uri_of(&project, "lib/use.dart"),
    ];
    documents.open(uris[0].clone(), 1, MODEL.to_string());
    documents.open(uris[1].clone(), 1, USE.to_string());
    let mut analyzer = analyzer(&project);
    let report = analyzer.check_reference_index(&documents, &uris);
    assert_same(&report, 40);
}

#[test]
fn the_index_follows_body_and_api_edits() {
    let project = common::Project::new("reference-index-edits");
    project.write("lib/model.dart", MODEL);
    project.write("lib/use.dart", USE);
    let mut documents = DocumentStore::new();
    let uris = vec![
        uri_of(&project, "lib/model.dart"),
        uri_of(&project, "lib/use.dart"),
    ];
    documents.open(uris[0].clone(), 1, MODEL.to_string());
    documents.open(uris[1].clone(), 1, USE.to_string());
    let mut analyzer = analyzer(&project);
    assert_same(&analyzer.check_reference_index(&documents, &uris), 40);
    // A body edit in the user, then an API change in the model (a member
    // renamed): the stale records are made again before each answer.
    let edited = USE.replace("print(x.extra);", "print(x.extra); print(model.count);");
    documents.update(&uris[1], 2, edited);
    assert_same(&analyzer.compare_with_current_index(&documents, &uris), 40);
    let renamed = MODEL
        .replace("String describe()", "String depict()")
        .replace("describe().toUpperCase()", "depict().toUpperCase()");
    documents.update(&uris[0], 2, renamed);
    assert_same(&analyzer.compare_with_current_index(&documents, &uris), 30);
}
