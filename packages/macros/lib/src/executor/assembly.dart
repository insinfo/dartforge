/// Assembly of the augmentation from the phases' structured results. Ports
/// the contract of `crates/macros_host/src/assembly.rs` to the builder,
/// keeping the order of results, types, imports and code parts.
library;

/// What an identifier means in the code the macro produced.
enum IdentifierKind { topLevel, staticMember, instanceMember, local }

final class ResolvedIdentifier {
  final String name;
  final IdentifierKind kind;
  final String? uri;
  final String? scope;
  const ResolvedIdentifier(this.name, this.kind, {this.uri, this.scope});
}

final class AugmentedType {
  final String keyword;
  final List<String> modifiers;
  final String name;
  final List<Map<String, Object?>> parameters;
  const AugmentedType(this.keyword, this.modifiers, this.name, [this.parameters = const []]);
}

/// The semantic answers the assembly needs. The builder gets them from the
/// `Resolver`; native execution uses the Rust equivalent.
abstract interface class AssemblyResolver {
  ResolvedIdentifier identifier(int id);
  AugmentedType augmentedType(int id);
  Map<String, Object?>? inferredType(int key);
}

/// [results] are the maps of `Result.toJson()`, in phase and application
/// order. [header] is `augment library` or `part of`, and [uri] names the
/// original library as CFE does. Resolution failures are builder errors.
String assembleAugmentation(
  List<Map<String, Object?>> results,
  AssemblyResolver resolver, {
  required String header,
  required String uri,
}) {
  if (header != 'augment library' && header != 'part of') {
    throw ArgumentError.value(header, 'header');
  }
  final a = _Assembler(resolver);
  final values = <int, List<Map<String, Object?>>>{};
  final extends_ = <int, Map<String, Object?>>{};
  final interfaces = <int, List<Map<String, Object?>>>{};
  final mixins = <int, List<Map<String, Object?>>>{};
  final members = <int, List<Map<String, Object?>>>{};

  for (final result in results) {
    for (final code in _codes(result['library'])) {
      a.code(code);
      a.text('\n');
    }
    _accumulate(values, result['enumValues']);
    for (final pair in _pairs(result['extends'])) {
      final id = pair[0] as int;
      if (extends_.containsKey(id)) {
        throw StateError('A class cannot extend multiple classes: ${resolver.identifier(id).name}');
      }
      extends_[id] = _code(pair[1]);
    }
    _accumulate(interfaces, result['interfaces']);
    _accumulate(mixins, result['mixins']);
    _accumulate(members, result['types']);
  }

  // The first occurrence per category sets the order of the types.
  final types = <int>{...values.keys, ...extends_.keys, ...interfaces.keys, ...mixins.keys, ...members.keys};
  for (final id in types) {
    final type = resolver.augmentedType(id);
    final modifiers = type.modifiers.isEmpty ? '' : '${type.modifiers.join(' ')} ';
    a.text('augment $modifiers${type.keyword} ${type.name}${type.parameters.isEmpty ? ' ' : ''}');
    if (type.parameters.isNotEmpty) {
      a.text('<');
      for (var i = 0; i < type.parameters.length; i++) {
        if (i > 0) a.text(', ');
        a.code(type.parameters[i]);
      }
      a.text('> ');
    }
    if (extends_.containsKey(id)) {
      a.text('extends ');
      a.code(extends_[id]!);
      a.text(' ');
    }
    for (final (name, codes) in [('with', mixins[id]), ('implements', interfaces[id])]) {
      if (codes == null || codes.isEmpty) continue;
      a.text('$name ');
      for (var i = 0; i < codes.length; i++) {
        if (i > 0) a.text(', ');
        a.code(codes[i]);
      }
      a.text(' ');
    }
    a.text('{\n');
    if (type.keyword == 'enum') {
      for (final code in values[id] ?? const <Map<String, Object?>>[]) {
        a.code(code);
      }
      a.text(';\n');
    }
    for (final code in members[id] ?? const <Map<String, Object?>>[]) {
      a.code(code);
      a.text('\n');
    }
    a.text('}\n');
  }
  return a.finish(header, uri);
}

List<List<Object?>> _pairs(Object? value) => [
      for (final pair in value as List? ?? const []) List<Object?>.from(pair as List),
    ];

Map<String, Object?> _code(Object? value) => Map<String, Object?>.from(value as Map);

List<Map<String, Object?>> _codes(Object? value) => [
      for (final code in value as List? ?? const []) _code(code),
    ];

void _accumulate(Map<int, List<Map<String, Object?>>> target, Object? pairs) {
  for (final pair in _pairs(pairs)) {
    (target[pair[0] as int] ??= []).addAll(_codes(pair[1]));
  }
}

final class _Part {
  final String? text;
  final int? name;
  const _Part.text(String this.text) : name = null;
  const _Part.name(int this.name) : text = null;

  String resolve(List<String> names) => text ?? names[name!];
}

final class _Assembler {
  final AssemblyResolver resolver;
  final imports = <String, int>{};
  final names = <String>[];
  final importParts = <_Part>[];
  final parts = <_Part>[];
  final texts = <String>[];
  final buffer = <String>[];
  String last = '';

  _Assembler(this.resolver);

  void text(String value) {
    last = value;
    buffer.add(value);
  }

  void _flush() {
    for (final value in buffer) {
      parts.add(_Part.text(value));
      texts.add(value);
    }
    buffer.clear();
  }

  void _name(int index) {
    _flush();
    last = '';
    parts.add(_Part.name(index));
  }

  void _ident(int id) {
    final ident = resolver.identifier(id);
    final import = ident.uri == null
        ? null
        : imports.putIfAbsent(ident.uri!, () {
            final index = names.length;
            names.add('');
            importParts.addAll([
              _Part.text("import '${ident.uri}' as "),
              _Part.name(index),
              const _Part.text(';\n'),
            ]);
            return index;
          });
    if (ident.kind == IdentifierKind.instanceMember) {
      if (!last.trimRight().endsWith('.')) text('this.');
    } else if (import != null) {
      _name(import);
      text('.');
    }
    if (ident.kind == IdentifierKind.staticMember) text('${ident.scope ?? ''}.');
    text(ident.name);
  }

  void code(Map<String, Object?> code) {
    for (final part in code['p'] as List) {
      switch (part) {
        case String value:
          text(value);
        case {'i': int id}:
          _ident(id);
        case {'o': int key}:
          final inferred = resolver.inferredType(key);
          if (inferred == null) throw StateError('no inferred type for the omitted type $key');
          this.code(inferred);
        case Map other:
          this.code(Map<String, Object?>.from(other));
        default:
          throw FormatException('invalid code part: $part');
      }
    }
  }

  String finish(String header, String uri) {
    _flush();
    if (imports.isNotEmpty) {
      final base = _freshPrefix(texts, 'prefix');
      for (var i = 0; i < imports.length; i++) {
        names[i] = '$base$i';
      }
    }
    final out = StringBuffer("$header '$uri';\n\n");
    for (final part in importParts) {
      out.write(part.resolve(names));
    }
    if (importParts.isNotEmpty) out.write('\n');
    for (final part in parts) {
      out.write(part.resolve(names));
    }
    return out.toString();
  }
}

String _freshPrefix(List<String> texts, String name) {
  var index = -1;
  var prefix = name;
  for (final text in texts) {
    while (text.contains(prefix)) {
      index++;
      prefix = '$name$index';
    }
  }
  return index > 0 ? '${prefix}_' : prefix;
}
