part of '../api.dart';

// Código que uma macro monta (spec, "Code objects"). Um [Code] é uma lista
// de partes — `String`, [Identifier], [Code] aninhado e, dentro de um
// [OmittedTypeAnnotationCode], [OmittedTypeAnnotation] — que o hospedeiro
// transforma em texto: um [Identifier] vira o nome com o prefixo de import
// certo, e o tipo omitido vira o tipo inferido. A ordem e o texto exato das
// partes de cada subtipo fazem parte do contrato: o texto da biblioteca de
// augmentation é conferido byte a byte com o do CFE 3.6.2.

/// Um pedaço de código Dart, válido ou não.
sealed class Code {
  /// As partes: `String`, [Identifier], [Code] (e [OmittedTypeAnnotation]
  /// dentro de [OmittedTypeAnnotationCode]).
  final List<Object> parts;

  /// O tipo do código, para `switch` sem testes de tipo.
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

/// Código arbitrário, sem exigência sintática; serve para compor outros.
final class RawCode extends Code {
  @override
  CodeKind get kind => CodeKind.raw;

  RawCode.fromString(super.code) : super.fromString();

  RawCode.fromParts(super.parts) : super.fromParts();
}

/// Uma declaração sintaticamente válida.
final class DeclarationCode extends Code {
  @override
  CodeKind get kind => CodeKind.declaration;

  DeclarationCode.fromString(super.code) : super.fromString();

  DeclarationCode.fromParts(super.parts) : super.fromParts();
}

/// Um comentário (de documentação, com referências entre `[]`).
final class CommentCode extends Code {
  @override
  CodeKind get kind => CodeKind.comment;

  CommentCode.fromString(super.code) : super.fromString();

  CommentCode.fromParts(super.parts) : super.fromParts();
}

/// Uma expressão sintaticamente válida.
final class ExpressionCode extends Code {
  @override
  CodeKind get kind => CodeKind.expression;

  ExpressionCode.fromString(super.code) : super.fromString();

  ExpressionCode.fromParts(super.parts) : super.fromParts();
}

/// Um corpo de função: tudo depois da lista de parâmetros, modificadores
/// (`async`…) incluídos; bloco ou `=>`.
final class FunctionBodyCode extends Code {
  @override
  CodeKind get kind => CodeKind.functionBody;

  FunctionBodyCode.fromString(super.code) : super.fromString();

  FunctionBodyCode.fromParts(super.parts) : super.fromParts();
}

/// Um parâmetro de função ou de tipo de função (posicional ou nomeado: quem
/// monta a lista decide).
final class ParameterCode implements Code {
  /// Valor padrão, sem o ` = `.
  final Code? defaultValue;

  /// Palavras antes do tipo (`required`, `final`…).
  final List<String> keywords;

  /// Pode faltar só em tipo de função.
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

/// Uma anotação de tipo.
sealed class TypeAnnotationCode implements Code, TypeAnnotation {
  @override
  TypeAnnotationCode get code => this;

  /// A versão não anulável (o próprio objeto, se já é).
  TypeAnnotationCode get asNonNullable => this;

  /// A versão anulável.
  NullableTypeAnnotationCode get asNullable => NullableTypeAnnotationCode(this);

  @override
  bool get isNullable => false;
}

/// `T?`.
final class NullableTypeAnnotationCode implements TypeAnnotationCode {
  /// O tipo que fica anulável.
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

/// Um tipo nomeado com argumentos: `Nome<A, B>`.
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

/// Um tipo de função: `R Function<T>(A, [B], {C c})`.
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

/// Um campo de tipo record (só dentro de [RecordTypeAnnotationCode]).
final class RecordFieldCode implements Code {
  /// Posicionais podem não ter nome.
  final String? name;

  final TypeAnnotationCode type;

  @override
  CodeKind get kind => CodeKind.recordField;

  @override
  List<Object> get parts => [type, if (name != null) ' ${name!}'];

  RecordFieldCode({this.name, required this.type});
}

/// Um tipo record: `(A, B a, {C c})`.
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

/// Um tipo omitido: vira o tipo inferido quando o texto é montado.
final class OmittedTypeAnnotationCode extends TypeAnnotationCode {
  final OmittedTypeAnnotation typeAnnotation;

  OmittedTypeAnnotationCode(this.typeAnnotation);

  @override
  CodeKind get kind => CodeKind.omittedTypeAnnotation;

  @override
  List<Object> get parts => [typeAnnotation];
}

/// Tipo escrito cru (para um tipo local sem [Identifier], como um recém
/// declarado). Prefira os subtipos específicos.
final class RawTypeAnnotationCode extends RawCode
    implements TypeAnnotationCode {
  @override
  CodeKind get kind => CodeKind.rawTypeAnnotation;

  @override
  TypeAnnotationCode get asNonNullable => this;

  @override
  NullableTypeAnnotationCode get asNullable => NullableTypeAnnotationCode(this);

  RawTypeAnnotationCode._(super.parts) : super.fromParts();

  /// De um texto sem espaço no fim.
  static TypeAnnotationCode fromString(String code) => fromParts([code]);

  /// De partes sem espaço no fim. Um `?` final vira
  /// [NullableTypeAnnotationCode] em volta do resto.
  static TypeAnnotationCode fromParts(List<Object> parts) {
    final (anulavel, resto) = _semInterrogacao(parts);
    final TypeAnnotationCode codigo = RawTypeAnnotationCode._(resto);
    return anulavel ? codigo.asNullable : codigo;
  }

  @override
  TypeAnnotationCode get code => this;

  @override
  bool get isNullable => false;

  /// Tira o `?` da última parte de texto não vazia, se houver. Espaço no fim
  /// é erro; um [Identifier] no fim encerra a busca (nunca tem `?`).
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

/// Um parâmetro de tipo: `T extends B`.
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
  /// Os itens intercalados com [separator], sem converter os itens em texto.
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
