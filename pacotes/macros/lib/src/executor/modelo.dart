/// O lado do executor: os objetos de introspecção montados a partir do
/// **modelo** que o hospedeiro envia (docs/MACROS-PROTOCOLO.md §5), e o
/// introspector que responde às consultas de uma macro — do modelo já
/// recebido quando dá (a pré-busca), do hospedeiro quando não.
///
/// Dart puro, sem nada que só o compilador do DartForge tenha: o mesmo
/// código roda no executor nativo e numa VM qualquer (o builder de
/// materialização, docs/MACROS-COMPATIBILIDADE.md).
library;

import 'dart:async';

import '../api.dart';

/// Quem responde às consultas: o hospedeiro pelo protocolo `macro.*` do
/// `dfexec/1`, ou, nos testes, um universo em memória.
abstract interface class Hospedeiro {
  /// Uma consulta [tipo] com [args]; a resposta é o `valor` do protocolo.
  /// Erro do hospedeiro vira [ErroDoHospedeiro].
  Future<Object?> consultar(String tipo, Map<String, Object?> args);
}

/// Resposta de erro do hospedeiro (`{"erro": {"tipo", "mensagem"}}`).
class ErroDoHospedeiro implements Exception {
  /// `implementacao` (uso errado da API), `ciclo` (ciclo de introspecção na
  /// fase de declarações) ou `inesperado`.
  final String tipo;
  final String mensagem;
  ErroDoHospedeiro(this.tipo, this.mensagem);

  /// A exceção da API que a macro vê.
  MacroException comoExcecaoDeMacro() => switch (tipo) {
        'implementacao' => ExcecaoDeImplementacao(mensagem),
        'ciclo' => ExcecaoDeCiclo(mensagem),
        _ => ExcecaoInesperada(mensagem),
      };

  @override
  String toString() => 'ErroDoHospedeiro($tipo): $mensagem';
}

// ---------------------------------------------------------------- exceções

abstract class _ExcecaoBase implements MacroException {
  @override
  final String message;
  @override
  final String? stackTrace;
  _ExcecaoBase(this.message, [this.stackTrace]);

  /// O nome no protocolo (`resultado.excecao.tipo`).
  String get tipo;

  @override
  String toString() => '$runtimeType: $message';
}

class ExcecaoDeImplementacao extends _ExcecaoBase
    implements MacroImplementationException {
  ExcecaoDeImplementacao(super.message, [super.stackTrace]);
  @override
  String get tipo => 'implementacao';
}

class ExcecaoDeCiclo extends _ExcecaoBase
    implements MacroIntrospectionCycleException {
  ExcecaoDeCiclo(super.message, [super.stackTrace]);
  @override
  String get tipo => 'ciclo';
}

class ExcecaoInesperada extends _ExcecaoBase
    implements UnexpectedMacroException {
  ExcecaoInesperada(super.message, [super.stackTrace]);
  @override
  String get tipo => 'inesperada';
}

/// O tipo no protocolo de uma [MacroException] qualquer.
String tipoDaExcecao(MacroException e) => switch (e) {
      _ExcecaoBase() => e.tipo,
      MacroIntrospectionCycleException() => 'ciclo',
      MacroImplementationException() => 'implementacao',
      _ => 'inesperada',
    };

// ------------------------------------------------------------ identificadores

/// Um identificador do hospedeiro: o `id` é a chave com que ele o resolve na
/// montagem do texto. Um objeto por `id` em cada [Modelo], então `==` é
/// identidade (como a macro espera ao procurar um membro pelo identificador).
final class IdentificadorImpl implements Identifier {
  final int id;
  @override
  final String name;
  IdentificadorImpl(this.id, this.name);

  @override
  String toString() => 'Identifier($name#$id)';
}

// ------------------------------------------------------------------- tipos

sealed class AnotacaoImpl implements TypeAnnotation {
  /// Posição da anotação no fonte, quando o hospedeiro a deu (alvo de
  /// diagnóstico).
  final int? id;
  @override
  final bool isNullable;
  AnotacaoImpl(this.id, this.isNullable);
}

final class TipoNomeadoImpl extends AnotacaoImpl implements NamedTypeAnnotation {
  @override
  final IdentificadorImpl identifier;
  @override
  final List<AnotacaoImpl> typeArguments;
  TipoNomeadoImpl(super.id, super.isNullable, this.identifier, this.typeArguments);

  @override
  TypeAnnotationCode get code {
    final base = NamedTypeAnnotationCode(
        name: identifier, typeArguments: [for (final a in typeArguments) a.code]);
    return isNullable ? base.asNullable : base;
  }
}

final class TipoFuncaoImpl extends AnotacaoImpl implements FunctionTypeAnnotation {
  @override
  final AnotacaoImpl returnType;
  @override
  final List<ParametroImpl> positionalParameters;
  @override
  final List<ParametroImpl> namedParameters;
  @override
  final List<ParametroDeTipoImpl> typeParameters;
  TipoFuncaoImpl(super.id, super.isNullable, this.returnType, this.positionalParameters,
      this.namedParameters, this.typeParameters);

  @override
  TypeAnnotationCode get code {
    final base = FunctionTypeAnnotationCode(
      returnType: returnType.code,
      typeParameters: [for (final t in typeParameters) t.code],
      positionalParameters: [
        for (final p in positionalParameters)
          if (p.isRequired) p.code
      ],
      optionalPositionalParameters: [
        for (final p in positionalParameters)
          if (!p.isRequired) p.code
      ],
      namedParameters: [for (final p in namedParameters) p.code],
    );
    return isNullable ? base.asNullable : base;
  }
}

final class TipoRecordImpl extends AnotacaoImpl implements RecordTypeAnnotation {
  @override
  final List<CampoDeRecordImpl> positionalFields;
  @override
  final List<CampoDeRecordImpl> namedFields;
  TipoRecordImpl(super.id, super.isNullable, this.positionalFields, this.namedFields);

  @override
  TypeAnnotationCode get code {
    final base = RecordTypeAnnotationCode(
      namedFields: [for (final c in namedFields) c.code],
      positionalFields: [for (final c in positionalFields) c.code],
    );
    return isNullable ? base.asNullable : base;
  }
}

final class TipoOmitidoImpl extends AnotacaoImpl implements OmittedTypeAnnotation {
  /// Chave do tipo omitido no hospedeiro (`inferType` e a montagem).
  final int chave;
  TipoOmitidoImpl(this.chave) : super(null, false);

  @override
  TypeAnnotationCode get code => OmittedTypeAnnotationCode(this);
}

final class CampoDeRecordImpl implements RecordField {
  @override
  final String? name;
  @override
  final AnotacaoImpl type;
  CampoDeRecordImpl(this.name, this.type);

  @override
  RecordFieldCode get code => RecordFieldCode(type: type.code, name: name);
}

/// Parâmetro de tipo de função (sem declaração).
final class ParametroImpl implements FormalParameter {
  @override
  final String? name;
  @override
  final AnotacaoImpl type;
  @override
  final bool isNamed;
  @override
  final bool isRequired;
  @override
  final ParameterStyle style;
  @override
  final List<MetadataAnnotation> metadata;
  ParametroImpl(this.name, this.type, this.isNamed, this.isRequired, this.style, this.metadata);

  @override
  ParameterCode get code => ParameterCode(
      name: name, type: type.code, keywords: [if (isNamed && isRequired) 'required']);
}

/// Parâmetro de tipo de tipo de função (sem declaração).
final class ParametroDeTipoImpl implements TypeParameter {
  @override
  final String name;
  @override
  final AnotacaoImpl? bound;
  @override
  final List<MetadataAnnotation> metadata;
  ParametroDeTipoImpl(this.name, this.bound, this.metadata);

  @override
  TypeParameterCode get code => TypeParameterCode(name: name, bound: bound?.code);
}

// ------------------------------------------------------------- bibliotecas

final class VersaoImpl implements LanguageVersion {
  @override
  final int major;
  @override
  final int minor;
  VersaoImpl(this.major, this.minor);
}

final class BibliotecaImpl implements Library {
  final int id;
  @override
  final Uri uri;
  @override
  final LanguageVersion languageVersion;
  @override
  final List<MetadataAnnotation> metadata;
  BibliotecaImpl(this.id, this.uri, this.languageVersion, this.metadata);
}

// -------------------------------------------------------------- anotações

final class AnotacaoIdentificadorImpl implements IdentifierMetadataAnnotation {
  @override
  final IdentificadorImpl identifier;
  AnotacaoIdentificadorImpl(this.identifier);
}

final class AnotacaoConstrutorImpl implements ConstructorMetadataAnnotation {
  @override
  final TipoNomeadoImpl type;
  @override
  final IdentificadorImpl constructor;
  @override
  final List<ExpressionCode> positionalArguments;
  @override
  final Map<String, ExpressionCode> namedArguments;
  AnotacaoConstrutorImpl(this.type, this.constructor, this.positionalArguments, this.namedArguments);
}

// ------------------------------------------------------------- declarações

/// Base de toda declaração vinda do modelo.
sealed class DeclaracaoImpl implements Declaration {
  @override
  final IdentificadorImpl identifier;
  @override
  final BibliotecaImpl library;
  @override
  final List<MetadataAnnotation> metadata;
  DeclaracaoImpl(this.identifier, this.library, this.metadata);
}

final class ParametroDeTipoDeclImpl extends DeclaracaoImpl implements TypeParameterDeclaration {
  @override
  final AnotacaoImpl? bound;
  ParametroDeTipoDeclImpl(super.identifier, super.library, super.metadata, this.bound);

  @override
  String get name => identifier.name;

  @override
  TypeParameterCode get code => TypeParameterCode(name: identifier.name, bound: bound?.code);
}

sealed class TipoDeclImpl extends DeclaracaoImpl implements ParameterizedTypeDeclaration {
  @override
  final List<ParametroDeTipoDeclImpl> typeParameters;
  TipoDeclImpl(super.identifier, super.library, super.metadata, this.typeParameters);
}

final class ClasseImpl extends TipoDeclImpl implements ClassDeclaration {
  @override
  final bool hasAbstract, hasBase, hasExternal, hasFinal, hasInterface, hasMixin, hasSealed;
  @override
  final TipoNomeadoImpl? superclass;
  @override
  final List<TipoNomeadoImpl> interfaces;
  @override
  final List<TipoNomeadoImpl> mixins;
  ClasseImpl(super.identifier, super.library, super.metadata, super.typeParameters,
      {required this.hasAbstract,
      required this.hasBase,
      required this.hasExternal,
      required this.hasFinal,
      required this.hasInterface,
      required this.hasMixin,
      required this.hasSealed,
      required this.superclass,
      required this.interfaces,
      required this.mixins});
}

final class EnumImpl extends TipoDeclImpl implements EnumDeclaration {
  @override
  final List<TipoNomeadoImpl> interfaces;
  @override
  final List<TipoNomeadoImpl> mixins;
  EnumImpl(super.identifier, super.library, super.metadata, super.typeParameters, this.interfaces, this.mixins);
}

final class MixinImpl extends TipoDeclImpl implements MixinDeclaration {
  @override
  final bool hasBase;
  @override
  final List<TipoNomeadoImpl> interfaces;
  @override
  final List<TipoNomeadoImpl> superclassConstraints;
  MixinImpl(super.identifier, super.library, super.metadata, super.typeParameters, this.hasBase,
      this.interfaces, this.superclassConstraints);
}

final class ExtensionImpl extends TipoDeclImpl implements ExtensionDeclaration {
  @override
  final AnotacaoImpl onType;
  ExtensionImpl(super.identifier, super.library, super.metadata, super.typeParameters, this.onType);
}

final class ExtensionTypeImpl extends TipoDeclImpl implements ExtensionTypeDeclaration {
  @override
  final AnotacaoImpl representationType;
  ExtensionTypeImpl(super.identifier, super.library, super.metadata, super.typeParameters, this.representationType);
}

final class TypedefImpl extends TipoDeclImpl implements TypeAliasDeclaration {
  @override
  final AnotacaoImpl aliasedType;
  TypedefImpl(super.identifier, super.library, super.metadata, super.typeParameters, this.aliasedType);
}

final class ValorDeEnumImpl extends DeclaracaoImpl implements EnumValueDeclaration {
  @override
  final IdentificadorImpl definingEnum;
  ValorDeEnumImpl(super.identifier, super.library, super.metadata, this.definingEnum);
}

final class ParametroDeclImpl extends DeclaracaoImpl implements FormalParameterDeclaration {
  @override
  final AnotacaoImpl type;
  @override
  final bool isNamed;
  @override
  final bool isRequired;
  @override
  final ParameterStyle style;
  ParametroDeclImpl(super.identifier, super.library, super.metadata, this.type, this.isNamed, this.isRequired, this.style);

  @override
  String get name => identifier.name;

  @override
  ParameterCode get code => ParameterCode(
      name: identifier.name,
      style: style,
      type: type.code,
      keywords: [if (isNamed && isRequired) 'required']);
}

/// Função de topo; base de método e construtor.
base class FuncaoImpl extends DeclaracaoImpl implements FunctionDeclaration {
  @override
  final bool hasBody, hasExternal, isOperator, isGetter, isSetter;
  @override
  final AnotacaoImpl returnType;
  @override
  final List<ParametroDeclImpl> positionalParameters;
  @override
  final List<ParametroDeclImpl> namedParameters;
  @override
  final List<ParametroDeTipoDeclImpl> typeParameters;
  FuncaoImpl(super.identifier, super.library, super.metadata,
      {required this.hasBody,
      required this.hasExternal,
      required this.isOperator,
      required this.isGetter,
      required this.isSetter,
      required this.returnType,
      required this.positionalParameters,
      required this.namedParameters,
      required this.typeParameters});
}

base class MetodoImpl extends FuncaoImpl implements MethodDeclaration {
  @override
  final IdentificadorImpl definingType;
  @override
  final bool hasStatic;
  MetodoImpl(super.identifier, super.library, super.metadata,
      {required super.hasBody,
      required super.hasExternal,
      required super.isOperator,
      required super.isGetter,
      required super.isSetter,
      required super.returnType,
      required super.positionalParameters,
      required super.namedParameters,
      required super.typeParameters,
      required this.definingType,
      required this.hasStatic});
}

final class ConstrutorImpl extends MetodoImpl implements ConstructorDeclaration {
  @override
  final bool isConst, isFactory;
  ConstrutorImpl(super.identifier, super.library, super.metadata,
      {required super.hasBody,
      required super.hasExternal,
      required super.returnType,
      required super.positionalParameters,
      required super.namedParameters,
      required super.typeParameters,
      required super.definingType,
      required this.isConst,
      required this.isFactory})
      : super(isOperator: false, isGetter: false, isSetter: false, hasStatic: true);
}

base class VariavelImpl extends DeclaracaoImpl implements VariableDeclaration {
  @override
  final bool hasConst, hasExternal, hasFinal, hasInitializer, hasLate;
  @override
  final AnotacaoImpl type;
  VariavelImpl(super.identifier, super.library, super.metadata,
      {required this.hasConst,
      required this.hasExternal,
      required this.hasFinal,
      required this.hasInitializer,
      required this.hasLate,
      required this.type});
}

final class CampoImpl extends VariavelImpl implements FieldDeclaration {
  @override
  final IdentificadorImpl definingType;
  @override
  final bool hasStatic, hasAbstract;
  CampoImpl(super.identifier, super.library, super.metadata,
      {required super.hasConst,
      required super.hasExternal,
      required super.hasFinal,
      required super.hasInitializer,
      required super.hasLate,
      required super.type,
      required this.definingType,
      required this.hasStatic,
      required this.hasAbstract});
}

// ----------------------------------------------------------------- tipos estáticos

final class TipoEstaticoImpl implements NamedStaticType {
  /// Chave do tipo no hospedeiro.
  final int chave;
  final Modelo _modelo;
  final ParameterizedTypeDeclaration? _declaracao;
  final List<StaticType> _argumentos;
  TipoEstaticoImpl(this.chave, this._modelo, this._declaracao, this._argumentos);

  @override
  ParameterizedTypeDeclaration get declaration =>
      _declaracao ?? (throw ExcecaoDeImplementacao('o tipo não é nomeado'));

  @override
  List<StaticType> get typeArguments => _argumentos;

  @override
  Future<bool> isExactly(TipoEstaticoImpl other) async =>
      await _modelo.consultar('ehExatamente', {'a': chave, 'b': other.chave}) as bool;

  @override
  Future<bool> isSubtypeOf(TipoEstaticoImpl other) async =>
      await _modelo.consultar('ehSubtipo', {'a': chave, 'b': other.chave}) as bool;

  @override
  Future<NamedStaticType?> asInstanceOf(TypeDeclaration declaration) async {
    final r = await _modelo.consultar('comoInstanciaDe',
        {'tipo': chave, 'declaracao': (declaration as DeclaracaoImpl).identifier.id});
    return r == null ? null : _modelo.tipoEstatico(r as Map<String, Object?>);
  }
}

// -------------------------------------------------------------------- o modelo

/// Os objetos de uma sessão de execução: um por `id`, decodificados do JSON
/// do modelo, com as listas de membros já recebidas guardadas.
final class Modelo {
  final Hospedeiro hospedeiro;
  final _identificadores = <int, IdentificadorImpl>{};
  final _declaracoes = <int, DeclaracaoImpl>{};
  final _bibliotecas = <int, BibliotecaImpl>{};
  final _omitidos = <int, TipoOmitidoImpl>{};
  final _membros = <(int, String), List<DeclaracaoImpl>>{};

  Modelo(this.hospedeiro);

  Future<Object?> consultar(String tipo, Map<String, Object?> args) async {
    try {
      return await hospedeiro.consultar(tipo, args);
    } on ErroDoHospedeiro catch (e) {
      throw e.comoExcecaoDeMacro();
    }
  }

  /// Recebe a parte `modelo` de um `macro.executar`: bibliotecas,
  /// declarações e as listas de membros da pré-busca.
  void receber(Map<String, Object?> modelo) {
    for (final b in (modelo['bibliotecas'] as List? ?? const [])) {
      biblioteca(b as Map<String, Object?>);
    }
    for (final d in (modelo['declaracoes'] as List? ?? const [])) {
      declaracao(d as Map<String, Object?>);
    }
    final membros = modelo['membros'] as Map<String, Object?>? ?? const {};
    for (final MapEntry(key: dono, value: listas) in membros.entries) {
      for (final MapEntry(key: tipo, value: lista) in (listas as Map<String, Object?>).entries) {
        _membros[(int.parse(dono), tipo)] = [
          for (final d in lista as List) declaracao(d as Map<String, Object?>),
        ];
      }
    }
  }

  IdentificadorImpl identificador(Map<String, Object?> j) {
    final id = j['id'] as int;
    return _identificadores.putIfAbsent(id, () => IdentificadorImpl(id, j['nome'] as String));
  }

  BibliotecaImpl biblioteca(Object? j) {
    if (j is int) {
      return _bibliotecas[j] ?? (throw ExcecaoInesperada('biblioteca $j fora do modelo'));
    }
    final m = j as Map<String, Object?>;
    final id = m['id'] as int;
    return _bibliotecas.putIfAbsent(id, () {
      final v = m['versao'] as List;
      return BibliotecaImpl(id, Uri.parse(m['uri'] as String), VersaoImpl(v[0] as int, v[1] as int), []);
    });
  }

  AnotacaoImpl tipo(Map<String, Object?> j) {
    final id = j['id'] as int?;
    final anulavel = j['anulavel'] as bool? ?? false;
    return switch (j['t']) {
      'nomeado' => TipoNomeadoImpl(id, anulavel, identificador(j['ident'] as Map<String, Object?>),
          [for (final a in j['args'] as List? ?? const []) tipo(a as Map<String, Object?>)]),
      'funcao' => TipoFuncaoImpl(
          id,
          anulavel,
          tipo(j['retorno'] as Map<String, Object?>),
          [for (final p in j['posicionais'] as List? ?? const []) _parametro(p as Map<String, Object?>)],
          [for (final p in j['nomeados'] as List? ?? const []) _parametro(p as Map<String, Object?>)],
          [for (final t in j['tparams'] as List? ?? const []) _parametroDeTipo(t as Map<String, Object?>)]),
      'record' => TipoRecordImpl(
          id,
          anulavel,
          [for (final c in j['posicionais'] as List? ?? const []) _campoDeRecord(c as Map<String, Object?>)],
          [for (final c in j['nomeados'] as List? ?? const []) _campoDeRecord(c as Map<String, Object?>)]),
      'omitido' => _omitidos.putIfAbsent(j['chave'] as int, () => TipoOmitidoImpl(j['chave'] as int)),
      final t => throw ExcecaoInesperada('tipo desconhecido no modelo: $t'),
    };
  }

  ParametroImpl _parametro(Map<String, Object?> j) => ParametroImpl(
      j['nome'] as String?,
      tipo(j['tipo'] as Map<String, Object?>),
      j['nomeado'] as bool? ?? false,
      j['obrigatorio'] as bool? ?? false,
      _estilo(j['estilo']),
      _metadados(j));

  ParametroDeTipoImpl _parametroDeTipo(Map<String, Object?> j) => ParametroDeTipoImpl(
      j['nome'] as String,
      j['limite'] == null ? null : tipo(j['limite'] as Map<String, Object?>),
      _metadados(j));

  CampoDeRecordImpl _campoDeRecord(Map<String, Object?> j) =>
      CampoDeRecordImpl(j['nome'] as String?, tipo(j['tipo'] as Map<String, Object?>));

  ParameterStyle _estilo(Object? e) => switch (e) {
        'this' => ParameterStyle.fieldFormal,
        'super' => ParameterStyle.superFormal,
        _ => ParameterStyle.normal,
      };

  List<MetadataAnnotation> _metadados(Map<String, Object?> j) => [
        for (final m in j['meta'] as List? ?? const [])
          switch ((m as Map<String, Object?>)['k']) {
            'ident' => AnotacaoIdentificadorImpl(identificador(m['ident'] as Map<String, Object?>)),
            _ => AnotacaoConstrutorImpl(
                tipo(m['tipo'] as Map<String, Object?>) as TipoNomeadoImpl,
                identificador(m['construtor'] as Map<String, Object?>),
                [for (final a in m['posicionais'] as List? ?? const []) codigoDeJson(a, this) as ExpressionCode],
                {
                  for (final MapEntry(:key, :value) in (m['nomeados'] as Map<String, Object?>? ?? const {}).entries)
                    key: codigoDeJson(value, this) as ExpressionCode,
                }),
          },
      ];

  bool _b(Map<String, Object?> j, String k) => j[k] as bool? ?? false;

  List<TipoNomeadoImpl> _nomeados(Object? l) =>
      [for (final t in l as List? ?? const []) tipo(t as Map<String, Object?>) as TipoNomeadoImpl];

  List<ParametroDeTipoDeclImpl> _tparams(Map<String, Object?> j) =>
      [for (final t in j['tparams'] as List? ?? const []) declaracao(t as Map<String, Object?>) as ParametroDeTipoDeclImpl];

  List<ParametroDeclImpl> _pdecls(Object? l) =>
      [for (final p in l as List? ?? const []) declaracao(p as Map<String, Object?>) as ParametroDeclImpl];

  /// A declaração de `j`, uma por identificador.
  DeclaracaoImpl declaracao(Map<String, Object?> j) {
    final ident = identificador(j['ident'] as Map<String, Object?>);
    final ja = _declaracoes[ident.id];
    if (ja != null) return ja;
    final lib = biblioteca(j['lib']);
    final meta = _metadados(j);
    IdentificadorImpl dono() => identificador(j['dono'] as Map<String, Object?>);
    final DeclaracaoImpl d = switch (j['k']) {
      'classe' => ClasseImpl(ident, lib, meta, _tparams(j),
          hasAbstract: _b(j, 'abstract'),
          hasBase: _b(j, 'base'),
          hasExternal: _b(j, 'external'),
          hasFinal: _b(j, 'final'),
          hasInterface: _b(j, 'interface'),
          hasMixin: _b(j, 'mixin'),
          hasSealed: _b(j, 'sealed'),
          superclass: j['superclasse'] == null ? null : tipo(j['superclasse'] as Map<String, Object?>) as TipoNomeadoImpl,
          interfaces: _nomeados(j['interfaces']),
          mixins: _nomeados(j['mixins'])),
      'enum' => EnumImpl(ident, lib, meta, _tparams(j), _nomeados(j['interfaces']), _nomeados(j['mixins'])),
      'mixin' => MixinImpl(ident, lib, meta, _tparams(j), _b(j, 'base'), _nomeados(j['interfaces']),
          _nomeados(j['restricoes'])),
      'extension' => ExtensionImpl(ident, lib, meta, _tparams(j), tipo(j['sobre'] as Map<String, Object?>)),
      'extensionType' =>
        ExtensionTypeImpl(ident, lib, meta, _tparams(j), tipo(j['representacao'] as Map<String, Object?>)),
      'typedef' => TypedefImpl(ident, lib, meta, _tparams(j), tipo(j['alias'] as Map<String, Object?>)),
      'tparam' => ParametroDeTipoDeclImpl(
          ident, lib, meta, j['limite'] == null ? null : tipo(j['limite'] as Map<String, Object?>)),
      'valorEnum' => ValorDeEnumImpl(ident, lib, meta, identificador(j['enum'] as Map<String, Object?>)),
      'parametro' => ParametroDeclImpl(ident, lib, meta, tipo(j['tipo'] as Map<String, Object?>), _b(j, 'nomeado'),
          _b(j, 'obrigatorio'), _estilo(j['estilo'])),
      'funcao' => FuncaoImpl(ident, lib, meta,
          hasBody: _b(j, 'corpo'),
          hasExternal: _b(j, 'external'),
          isOperator: _b(j, 'operador'),
          isGetter: _b(j, 'getter'),
          isSetter: _b(j, 'setter'),
          returnType: tipo(j['retorno'] as Map<String, Object?>),
          positionalParameters: _pdecls(j['posicionais']),
          namedParameters: _pdecls(j['nomeados']),
          typeParameters: _tparams(j)),
      'metodo' => MetodoImpl(ident, lib, meta,
          hasBody: _b(j, 'corpo'),
          hasExternal: _b(j, 'external'),
          isOperator: _b(j, 'operador'),
          isGetter: _b(j, 'getter'),
          isSetter: _b(j, 'setter'),
          returnType: tipo(j['retorno'] as Map<String, Object?>),
          positionalParameters: _pdecls(j['posicionais']),
          namedParameters: _pdecls(j['nomeados']),
          typeParameters: _tparams(j),
          definingType: dono(),
          hasStatic: _b(j, 'static')),
      'construtor' => ConstrutorImpl(ident, lib, meta,
          hasBody: _b(j, 'corpo'),
          hasExternal: _b(j, 'external'),
          returnType: tipo(j['retorno'] as Map<String, Object?>),
          positionalParameters: _pdecls(j['posicionais']),
          namedParameters: _pdecls(j['nomeados']),
          typeParameters: _tparams(j),
          definingType: dono(),
          isConst: _b(j, 'const'),
          isFactory: _b(j, 'factory')),
      'variavel' => VariavelImpl(ident, lib, meta,
          hasConst: _b(j, 'const'),
          hasExternal: _b(j, 'external'),
          hasFinal: _b(j, 'final'),
          hasInitializer: _b(j, 'inicializador'),
          hasLate: _b(j, 'late'),
          type: tipo(j['tipo'] as Map<String, Object?>)),
      'campo' => CampoImpl(ident, lib, meta,
          hasConst: _b(j, 'const'),
          hasExternal: _b(j, 'external'),
          hasFinal: _b(j, 'final'),
          hasInitializer: _b(j, 'inicializador'),
          hasLate: _b(j, 'late'),
          type: tipo(j['tipo'] as Map<String, Object?>),
          definingType: dono(),
          hasStatic: _b(j, 'static'),
          hasAbstract: _b(j, 'abstract')),
      final k => throw ExcecaoInesperada('declaração desconhecida no modelo: $k'),
    };
    _declaracoes[ident.id] = d;
    return d;
  }

  TipoEstaticoImpl tipoEstatico(Map<String, Object?> j) {
    final decl = j['declaracao'];
    return TipoEstaticoImpl(
      j['chave'] as int,
      this,
      decl == null ? null : declaracao(decl as Map<String, Object?>) as ParameterizedTypeDeclaration,
      [for (final a in j['args'] as List? ?? const []) tipoEstatico(a as Map<String, Object?>)],
    );
  }

  /// Membros de [dono] (`campos`, `metodos`, `construtores`, `valores`):
  /// da pré-busca, senão do hospedeiro. O hospedeiro os manda em ordem
  /// lexicográfica de nome.
  Future<List<DeclaracaoImpl>> membros(IdentificadorImpl dono, String tipo) async {
    final ja = _membros[(dono.id, tipo)];
    if (ja != null) return ja;
    final r = await consultar('membros', {'dono': dono.id, 'tipo': tipo}) as List;
    return _membros[(dono.id, tipo)] = [for (final d in r) declaracao(d as Map<String, Object?>)];
  }
}

// ------------------------------------------------------------------- Code ⇄ JSON

/// `Code` no protocolo: `{"k": tipo, "p": [partes]}`; parte = texto, `{"i":
/// id, "n": nome}` (identificador), `{"o": chave}` (tipo omitido) ou outro
/// código.
Map<String, Object?> codigoParaJson(Code c) => {
      'k': c.kind.name,
      'p': [for (final p in c.parts) _parteParaJson(p)],
    };

Object? _parteParaJson(Object p) => switch (p) {
      String() => p,
      IdentificadorImpl() => {'i': p.id, 'n': p.name},
      Identifier() => throw ExcecaoDeImplementacao(
          'identificador que não veio do hospedeiro: ${p.name} (${p.runtimeType})'),
      TipoOmitidoImpl() => {'o': p.chave},
      Code() => codigoParaJson(p),
      _ => throw ExcecaoDeImplementacao('parte de código inválida: ${p.runtimeType}'),
    };

/// Uma anotação de tipo em código (`TypeAnnotationCode`) no formato de tipo
/// do modelo, para `resolve`: a estrutura (nome, argumentos, `?`) que o
/// `Code` achatado perde. Código cru não é resolvível (spec: `resolve` lança
/// para [RawTypeAnnotationCode]).
Map<String, Object?> tipoCodigoParaJson(TypeAnnotationCode c) => switch (c) {
      NullableTypeAnnotationCode(:final underlyingType) => {...tipoCodigoParaJson(underlyingType), 'anulavel': true},
      NamedTypeAnnotationCode(:final name, :final typeArguments) => {
          't': 'nomeado',
          'ident': switch (name) {
            IdentificadorImpl() => {'id': name.id, 'nome': name.name},
            _ => throw ExcecaoDeImplementacao('identificador que não veio do hospedeiro: ${name.name}'),
          },
          'args': [for (final a in typeArguments) tipoCodigoParaJson(a)],
        },
      FunctionTypeAnnotationCode() => {
          't': 'funcao',
          'texto': [for (final p in c.parts) _parteParaJson(p)],
        },
      RecordTypeAnnotationCode() => {
          't': 'record',
          'texto': [for (final p in c.parts) _parteParaJson(p)],
        },
      OmittedTypeAnnotationCode(:final typeAnnotation) => {
          't': 'omitido',
          'chave': (typeAnnotation as TipoOmitidoImpl).chave,
        },
      RawTypeAnnotationCode() => throw ExcecaoDeImplementacao(
          'resolve não aceita RawTypeAnnotationCode: use um subtipo específico de TypeAnnotationCode'),
    };

/// O inverso de [codigoParaJson], para argumentos de anotação.
Code codigoDeJson(Object? j, Modelo m) {
  final mapa = j as Map<String, Object?>;
  final partes = <Object>[
    for (final p in mapa['p'] as List)
      switch (p) {
        String() => p,
        {'i': final int id, 'n': final String n} => m.identificador({'id': id, 'nome': n}),
        {'o': final int chave} => m.tipo({'t': 'omitido', 'chave': chave}),
        _ => codigoDeJson(p, m),
      },
  ];
  return switch (mapa['k']) {
    'expression' => ExpressionCode.fromParts(partes),
    'declaration' => DeclarationCode.fromParts(partes),
    'functionBody' => FunctionBodyCode.fromParts(partes),
    'comment' => CommentCode.fromParts(partes),
    _ => RawCode.fromParts(partes),
  };
}
