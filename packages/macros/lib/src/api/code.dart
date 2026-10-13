part of '../api.dart';

// Code a macro assembles (spec, "Code objects"). A [Code] is a list of
// parts — `String`, [Identifier], nested [Code] and, inside an
// [OmittedTypeAnnotationCode], [OmittedTypeAnnotation] — that the host
// turns into text: an [Identifier] becomes the name with the right import
// prefix, and the omitted type becomes the inferred type. The order and exact
// text of each subtype's parts are part of the contract: the text of the
// augmentation library is checked byte for byte against CFE 3.6.2's.

/// A piece of Dart code, valid or not.
sealed class Code {
  /// The parts: `String`, [Identifier], [Code] (and [OmittedTypeAnnotation]
  /// inside [OmittedTypeAnnotationCode]).
  final List<Object> parts;

  /// The kind of code, for a `switch` without type tests.
  CodeKind get kind;

  Code.fromString(String code) : parts = [code];

  Code.fromParts(this.parts) {
    for (final parte in parts) {
      if (parte is! Code && parte is! Identifier && parte is! String) {
        throw StateError('Unrecognized code part ${parte.runtimeType}');
      }
    }
  }
}

/// Arbitrary code, with no syntactic requirement; used to compose others.
final class RawCode extends Code {
  @override
  CodeKind get kind => CodeKind.raw;

  RawCode.fromString(super.code) : super.fromString();

  RawCode.fromParts(super.parts) : super.fromParts();
}

/// A syntactically valid declaration.
final class DeclarationCode extends Code {
  @override
  CodeKind get kind => CodeKind.declaration;

  DeclarationCode.fromString(super.code) : super.fromString();

  DeclarationCode.fromParts(super.parts) : super.fromParts();
}

/// A comment (a doc comment, with references between `[]`).
final class CommentCode extends Code {
  @override
  CodeKind get kind => CodeKind.comment;

  CommentCode.fromString(super.code) : super.fromString();

  CommentCode.fromParts(super.parts) : super.fromParts();
}

/// A syntactically valid expression.
final class ExpressionCode extends Code {
  @override
  CodeKind get kind => CodeKind.expression;

  ExpressionCode.fromString(super.code) : super.fromString();

  ExpressionCode.fromParts(super.parts) : super.fromParts();
}

/// A function body: everything after the parameter list, modifiers
/// (`async`…) included; a block or `=>`.
final class FunctionBodyCode extends Code {
  @override
  CodeKind get kind => CodeKind.functionBody;

  FunctionBodyCode.fromString(super.code) : super.fromString();

  FunctionBodyCode.fromParts(super.parts) : super.fromParts();
}

/// A parameter of a function or of a function type (positional or named:
/// whoever assembles the list decides).
final class ParameterCode implements Code {
  /// Default value, without the ` = `.
  final Code? defaultValue;

  /// Words before the type (`required`, `final`…).
  final List<String> keywords;

  /// May be missing only in a function type.
  final String? name;

  final TypeAnnotationCode? type;

  final ParameterStyle style;

  @override
  CodeKind get kind => CodeKind.parameter;

  @override
  List<Object> get parts => [
        if (keywords.isNotEmpty) ...[...keywords.joinAsCode(' '), ' '],
        if (type != null) ...[type!, ' '],
        if (style == ParameterStyle.fieldFormal) 'this.',
        if (style == ParameterStyle.superFormal) 'super.',
        if (name != null) name!,
        if (defaultValue != null) ...[' = ', defaultValue!],
      ];

  ParameterCode({
    this.defaultValue,
    this.keywords = const [],
    this.name,
    this.style = ParameterStyle.normal,
    this.type,
  });
}

/// A type annotation.
sealed class TypeAnnotationCode implements Code, TypeAnnotation {
  @override
  TypeAnnotationCode get code => this;

  /// The non-nullable version (the object itself, if it already is).
  TypeAnnotationCode get asNonNullable => this;

  /// The nullable version.
  NullableTypeAnnotationCode get asNullable => NullableTypeAnnotationCode(this);

  @override
  bool get isNullable => false;
}

/// `T?`.
final class NullableTypeAnnotationCode implements TypeAnnotationCode {
  /// The type that becomes nullable.
  TypeAnnotationCode underlyingType;

  @override
  TypeAnnotationCode get code => this;

  @override
  CodeKind get kind => CodeKind.nullableTypeAnnotation;

  @override
  List<Object> get parts => [...underlyingType.parts, '?'];

  NullableTypeAnnotationCode(this.underlyingType);

  @override
  TypeAnnotationCode get asNonNullable => underlyingType;

  @override
  NullableTypeAnnotationCode get asNullable => this;

  @override
  bool get isNullable => true;
}

/// A named type with arguments: `Name<A, B>`.
final class NamedTypeAnnotationCode extends TypeAnnotationCode {
  final Identifier name;

  final List<TypeAnnotationCode> typeArguments;

  @override
  CodeKind get kind => CodeKind.namedTypeAnnotation;

  @override
  List<Object> get parts => [
        name,
        if (typeArguments.isNotEmpty) ...['<', ...typeArguments.joinAsCode(', '), '>'],
      ];

  NamedTypeAnnotationCode({required this.name, this.typeArguments = const []});
}

/// A function type: `R Function<T>(A, [B], {C c})`.
final class FunctionTypeAnnotationCode extends TypeAnnotationCode {
  final List<ParameterCode> namedParameters;

  final List<ParameterCode> optionalPositionalParameters;

  final List<ParameterCode> positionalParameters;

  final TypeAnnotationCode? returnType;

  final List<TypeParameterCode> typeParameters;

  @override
  CodeKind get kind => CodeKind.functionTypeAnnotation;

  @override
  List<Object> get parts => [
        if (returnType != null) returnType!,
        ' Function',
        if (typeParameters.isNotEmpty) ...['<', ...typeParameters.joinAsCode(', '), '>'],
        '(',
        for (final p in positionalParameters) ...[p, ', '],
        if (optionalPositionalParameters.isNotEmpty) ...[
          '[',
          for (final p in optionalPositionalParameters) ...[p, ', '],
          ']',
        ],
        if (namedParameters.isNotEmpty) ...[
          '{',
          for (final p in namedParameters) ...[p, ', '],
          '}',
        ],
        ')',
      ];

  FunctionTypeAnnotationCode({
    this.namedParameters = const [],
    this.optionalPositionalParameters = const [],
    this.positionalParameters = const [],
    this.returnType,
    this.typeParameters = const [],
  });
}

/// A field of a record type (only inside [RecordTypeAnnotationCode]).
final class RecordFieldCode implements Code {
  /// Positional ones may have no name.
  final String? name;

  final TypeAnnotationCode type;

  @override
  CodeKind get kind => CodeKind.recordField;

  @override
  List<Object> get parts => [type, if (name != null) ' ${name!}'];

  RecordFieldCode({this.name, required this.type});
}

/// A record type: `(A, B a, {C c})`.
final class RecordTypeAnnotationCode extends TypeAnnotationCode {
  final List<RecordFieldCode> namedFields;

  final List<RecordFieldCode> positionalFields;

  @override
  CodeKind get kind => CodeKind.recordTypeAnnotation;

  @override
  List<Object> get parts => [
        '(',
        for (final c in positionalFields) ...[
          if (c != positionalFields.first) ', ',
          c,
        ],
        if (namedFields.isNotEmpty) ...[
          if (positionalFields.isNotEmpty) ', ',
          '{',
          for (final c in namedFields) ...[
            if (c != namedFields.first) ', ',
            c,
          ],
          '}',
        ],
        ')',
      ];

  RecordTypeAnnotationCode({
    this.namedFields = const [],
    this.positionalFields = const [],
  });
}

/// An omitted type: becomes the inferred type when the text is assembled.
final class OmittedTypeAnnotationCode extends TypeAnnotationCode {
  final OmittedTypeAnnotation typeAnnotation;

  OmittedTypeAnnotationCode(this.typeAnnotation);

  @override
  CodeKind get kind => CodeKind.omittedTypeAnnotation;

  @override
  List<Object> get parts => [typeAnnotation];
}

/// A type written raw (for a local type without an [Identifier], like one just
/// declared). Prefer the specific subtypes.
final class RawTypeAnnotationCode extends RawCode
    implements TypeAnnotationCode {
  @override
  CodeKind get kind => CodeKind.rawTypeAnnotation;

  @override
  TypeAnnotationCode get asNonNullable => this;

  @override
  NullableTypeAnnotationCode get asNullable => NullableTypeAnnotationCode(this);

  RawTypeAnnotationCode._(super.parts) : super.fromParts();

  /// From a text with no trailing space.
  static TypeAnnotationCode fromString(String code) => fromParts([code]);

  /// From parts with no trailing space. A final `?` becomes
  /// [NullableTypeAnnotationCode] around the rest.
  static TypeAnnotationCode fromParts(List<Object> parts) {
    final (anulavel, resto) = _semInterrogacao(parts);
    final TypeAnnotationCode codigo = RawTypeAnnotationCode._(resto);
    return anulavel ? codigo.asNullable : codigo;
  }

  @override
  TypeAnnotationCode get code => this;

  @override
  bool get isNullable => false;

  /// Strips the `?` from the last non-empty text part, if any. Trailing space
  /// is an error; an [Identifier] at the end stops the search (it never has `?`).
  static (bool, List<Object>) _semInterrogacao(List<Object> parts) {
    for (var i = parts.length - 1; i >= 0; i--) {
      final parte = parts[i];
      if (parte is Identifier) return (false, parts);
      if (parte is! String) continue;
      if (parte.trimRight() != parte) {
        throw ArgumentError(
            'Invalid type annotation, type annotations should not end with '
            'whitespace but got `$parte`.');
      }
      if (parte.isEmpty) continue;
      if (!parte.endsWith('?')) return (false, parts);
      return (
        true,
        [...parts.sublist(0, i), parte.substring(0, parte.length - 1)],
      );
    }
    throw ArgumentError('The empty string is not a valid type annotation.');
  }
}

/// A type parameter: `T extends B`.
final class TypeParameterCode implements Code {
  final TypeAnnotationCode? bound;
  final String name;

  @override
  CodeKind get kind => CodeKind.typeParameter;

  @override
  List<Object> get parts => [name, if (bound != null) ...[' extends ', bound!]];

  TypeParameterCode({this.bound, required this.name});
}

extension Join<T extends Object> on List<T> {
  /// The items interleaved with [separator], without converting the items to text.
  List<Object> joinAsCode(String separator) => [
        for (var i = 0; i < length; i++) ...[
          if (i > 0) separator,
          this[i],
        ],
      ];
}

enum CodeKind {
  comment,
  declaration,
  expression,
  functionBody,
  functionTypeAnnotation,
  namedTypeAnnotation,
  nullableTypeAnnotation,
  omittedTypeAnnotation,
  parameter,
  raw,
  rawTypeAnnotation,
  recordField,
  recordTypeAnnotation,
  typeParameter,
}
