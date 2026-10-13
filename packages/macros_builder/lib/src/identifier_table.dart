/// Identifiers that stay stable across the macro applications of a library.
/// Mirrors the textual table of `macros_host::model::Table`: the same element
/// keeps its id across models and queries, even after reanalysis.
library;

import 'package:analyzer/dart/element/element.dart';

enum GeneratedCategory { method, staticMember, constructor, parameter, type }

final class GeneratedIdentifier {
  final String name;
  final String uri;
  final String? owner;
  final GeneratedCategory category;
  const GeneratedIdentifier(this.name, this.uri, this.category, {this.owner});
}

final class IdentifierTable {
  final _ids = <String, int>{};
  final _elements = <int, Element>{};
  final _generated = <int, GeneratedIdentifier>{};
  final _libraries = <String, int>{};
  final _omitted = <String, int>{};
  int _next = 1;

  int library(String uri) => _libraries.putIfAbsent(uri, () => _libraries.length + 1);

  int id(Element element) {
    final key = _key(element);
    final existing = _ids[key];
    if (existing != null) return existing;
    final id = _next++;
    _ids[key] = id;
    _elements[id] = element;
    return id;
  }

  void bind(Element element, int id) {
    final key = _key(element);
    final previous = _ids[key];
    if (previous != null && previous != id) {
      throw StateError('different id for $key: $previous and $id');
    }
    final other = _elements[id];
    if (other != null && _key(other) != key) {
      throw StateError('id $id already belongs to ${_key(other)}');
    }
    _ids[key] = id;
    _elements[id] = element;
    if (id >= _next) _next = id + 1;
  }

  Element? element(int id) => _elements[id];

  /// Declarations introduced by an augmentation do not have an Element yet in
  /// analyzer 7.3; the textual key keeps the id across phases.
  int generatedId(String key, {GeneratedIdentifier? meaning}) {
    final id = _ids.putIfAbsent('generated:$key', () => _next++);
    if (meaning != null) _generated[id] = meaning;
    return id;
  }

  GeneratedIdentifier? generated(int id) => _generated[id];

  int omitted(String key) => _omitted.putIfAbsent(key, () => _omitted.length + 1);

  String _key(Element element) {
    final uri = element.librarySource?.uri;
    final name = element.name;
    if (uri == null || name == null) {
      throw StateError('element without a URI or a name');
    }
    if (element is ClassElement) return 'type:$uri#$name';
    if (element is FieldElement) {
      final owner = element.enclosingElement3;
      if (owner is! ClassElement) {
        throw UnsupportedError('field without a class: $element');
      }
      return 'field:$uri#${owner.name}.$name';
    }
    throw UnsupportedError('identifier not covered yet: $element');
  }
}
