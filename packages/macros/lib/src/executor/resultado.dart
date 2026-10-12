/// O que uma aplicação de macro produz numa fase (o `MacroExecutionResult`
/// da spec) e os builders que o acumulam. O texto da biblioteca de
/// augmentation é montado **no hospedeiro** a partir deste resultado
/// estruturado (docs/MACROS-PROTOCOLO.md §4); aqui só se formam as
/// declarações `augment` que os builders da fase de definições geram, com o
/// mesmo texto que o CFE 3.6.2 gera (conferido byte a byte).
library;

import 'dart:async';

import '../api.dart';
import 'modelo.dart';

/// O resultado de uma aplicação numa fase. Os mapas guardam a ordem de
/// inserção: é a ordem em que o hospedeiro monta o texto (Regra 2).
final class Resultado {
  final diagnosticos = <Diagnostic>[];
  MacroException? excecao;
  final valoresDeEnum = <IdentificadorImpl, List<DeclarationCode>>{};
  final extends_ = <IdentificadorImpl, NamedTypeAnnotationCode>{};
  final interfaces = <IdentificadorImpl, List<TypeAnnotationCode>>{};
  final biblioteca = <DeclarationCode>[];
  final mixins = <IdentificadorImpl, List<TypeAnnotationCode>>{};
  final tiposNovos = <String>[];
  final tipos = <IdentificadorImpl, List<DeclarationCode>>{};

  /// O `resultado` do protocolo. Mapas viram listas de pares para a ordem
  /// sobreviver ao JSON.
  Map<String, Object?> paraJson() => {
        'diagnosticos': [for (final d in diagnosticos) _diagnostico(d)],
        'excecao': excecao == null
            ? null
            : {'tipo': tipoDaExcecao(excecao!), 'mensagem': excecao!.message, 'pilha': excecao!.stackTrace},
        'valoresDeEnum': _pares(valoresDeEnum, (l) => [for (final c in l) codigoParaJson(c)]),
        'extends': _pares(extends_, codigoParaJson),
        'interfaces': _pares(interfaces, (l) => [for (final c in l) codigoParaJson(c)]),
        'biblioteca': [for (final c in biblioteca) codigoParaJson(c)],
        'mixins': _pares(mixins, (l) => [for (final c in l) codigoParaJson(c)]),
        'tiposNovos': tiposNovos,
        'tipos': _pares(tipos, (l) => [for (final c in l) codigoParaJson(c)]),
      };

  static List<Object?> _pares<V>(Map<IdentificadorImpl, V> m, Object? Function(V) f) => [
        for (final MapEntry(:key, :value) in m.entries) [key.id, f(value)],
      ];

  static Map<String, Object?> _diagnostico(Diagnostic d) => {
        'severidade': d.severity.name,
        'mensagem': _mensagem(d.message),
        'contexto': [for (final m in d.contextMessages) _mensagem(m)],
        'correcao': d.correctionMessage,
      };

  static Map<String, Object?> _mensagem(DiagnosticMessage m) => {
        'texto': m.message,
        'alvo': switch (m.target) {
          null => null,
          DeclarationDiagnosticTarget(:final declaration) => {
              'declaracao': (declaration as DeclaracaoImpl).identifier.id
            },
          TypeAnnotationDiagnosticTarget(:final typeAnnotation) => {
              'anotacao': typeAnnotation is AnotacaoImpl ? typeAnnotation.id : null
            },
          MetadataAnnotationDiagnosticTarget() => {'metadado': null},
        },
      };
}

// ------------------------------------------------------------ builders

/// Base de todos os builders: o resultado compartilhado (um builder aninhado
/// — `buildMethod` etc. — escreve no mesmo resultado) e o introspector.
abstract class _Base implements Builder, TypePhaseIntrospector {
  final Resultado resultado;
  final Introspector introspector;
  _Base(this.resultado, this.introspector);

  @override
  void report(Diagnostic diagnostic) => resultado.diagnosticos.add(diagnostic);

  @override
  Future<Identifier> resolveIdentifier(Uri library, String name) =>
      // ignore: deprecated_member_use_from_same_package
      introspector.resolveIdentifier(library, name);
}

class ConstrutorDeTipos extends _Base implements TypeBuilder {
  ConstrutorDeTipos(super.resultado, super.introspector);

  @override
  void declareType(String name, DeclarationCode typeDeclaration) {
    resultado.tiposNovos.add(name);
    resultado.biblioteca.add(typeDeclaration);
  }
}

/// Fase de tipos de classe, enum e mixin: supertipos acrescentados ao alvo.
class ConstrutorDeTiposDoAlvo extends ConstrutorDeTipos
    implements ClassTypeBuilder, EnumTypeBuilder, MixinTypeBuilder {
  final IdentificadorImpl alvo;
  ConstrutorDeTiposDoAlvo(this.alvo, super.resultado, super.introspector);

  @override
  void extendsType(NamedTypeAnnotationCode superclass) {
    if (resultado.extends_.containsKey(alvo)) {
      throw ArgumentError.value(alvo.name, null, 'A type cannot extend multiple types');
    }
    resultado.extends_[alvo] = superclass;
  }

  @override
  void appendInterfaces(Iterable<TypeAnnotationCode> interfaces) =>
      (resultado.interfaces[alvo] ??= []).addAll(interfaces);

  @override
  void appendMixins(Iterable<TypeAnnotationCode> mixins) => (resultado.mixins[alvo] ??= []).addAll(mixins);
}

abstract class _BaseDeDeclaracoes extends _Base implements DeclarationPhaseIntrospector {
  _BaseDeDeclaracoes(super.resultado, super.introspector);

  @override
  Future<StaticType> resolve(TypeAnnotationCode type) => introspector.resolve(type);
  @override
  Future<List<EnumValueDeclaration>> valuesOf(EnumDeclaration enuum) => introspector.valuesOf(enuum);
  @override
  Future<List<FieldDeclaration>> fieldsOf(TypeDeclaration type) => introspector.fieldsOf(type);
  @override
  Future<List<MethodDeclaration>> methodsOf(TypeDeclaration type) => introspector.methodsOf(type);
  @override
  Future<List<ConstructorDeclaration>> constructorsOf(TypeDeclaration type) =>
      introspector.constructorsOf(type);
  @override
  Future<List<TypeDeclaration>> typesOf(Library library) => introspector.typesOf(library);
  @override
  Future<TypeDeclaration> typeDeclarationOf(Identifier identifier) =>
      introspector.typeDeclarationOf(identifier);
}

class ConstrutorDeDeclaracoes extends _BaseDeDeclaracoes implements DeclarationBuilder {
  ConstrutorDeDeclaracoes(super.resultado, super.introspector);

  @override
  void declareInLibrary(DeclarationCode declaration) => resultado.biblioteca.add(declaration);
}

/// Membros (e, para enum, valores) acrescentados a [tipo].
class ConstrutorDeMembros extends ConstrutorDeDeclaracoes implements EnumDeclarationBuilder {
  final IdentificadorImpl tipo;
  ConstrutorDeMembros(this.tipo, super.resultado, super.introspector);

  @override
  void declareInType(DeclarationCode declaration) => (resultado.tipos[tipo] ??= []).add(declaration);

  @override
  void declareEnumValue(DeclarationCode declaration) =>
      (resultado.valoresDeEnum[tipo] ??= []).add(declaration);
}

abstract class _BaseDeDefinicoes extends _BaseDeDeclaracoes implements DefinitionBuilder {
  _BaseDeDefinicoes(super.resultado, super.introspector);

  @override
  Future<Declaration> declarationOf(Identifier identifier) => introspector.declarationOf(identifier);
  @override
  Future<TypeAnnotation> inferType(OmittedTypeAnnotation omittedType) => introspector.inferType(omittedType);
  @override
  Future<List<Declaration>> topLevelDeclarationsOf(Library library) =>
      introspector.topLevelDeclarationsOf(library);

  /// O elemento de [lista] com o identificador [id] (a busca por identidade
  /// que os builders de membro fazem).
  T _com<T extends Declaration>(List<T> lista, Identifier id, String oque) {
    for (final d in lista) {
      if (d.identifier == id) return d;
    }
    throw ExcecaoDeImplementacao('$oque ${id.name} não encontrado');
  }
}

class ConstrutorDeDefinicaoDeTipo extends _BaseDeDefinicoes implements EnumDefinitionBuilder {
  final TypeDeclaration declaracao;
  ConstrutorDeDefinicaoDeTipo(this.declaracao, super.resultado, super.introspector);

  @override
  Future<ConstructorDefinitionBuilder> buildConstructor(Identifier identifier) async =>
      ConstrutorDeDefinicaoDeConstrutor(
          _com(await introspector.constructorsOf(declaracao), identifier, 'construtor') as ConstrutorImpl,
          resultado,
          introspector);

  @override
  Future<VariableDefinitionBuilder> buildField(Identifier identifier) async => ConstrutorDeDefinicaoDeVariavel(
      _com(await introspector.fieldsOf(declaracao), identifier, 'campo'), resultado, introspector);

  @override
  Future<FunctionDefinitionBuilder> buildMethod(Identifier identifier) async => ConstrutorDeDefinicaoDeFuncao(
      _com(await introspector.methodsOf(declaracao), identifier, 'método') as FuncaoImpl, resultado, introspector);

  @override
  Future<EnumValueDefinitionBuilder> buildEnumValue(Identifier identifier) async =>
      ConstrutorDeDefinicaoDeValor(
          _com(await introspector.valuesOf(declaracao as EnumDeclaration), identifier, 'valor')
              as ValorDeEnumImpl,
          resultado,
          introspector);
}

class ConstrutorDeDefinicaoDeBiblioteca extends _BaseDeDefinicoes implements LibraryDefinitionBuilder {
  final Library biblioteca;
  ConstrutorDeDefinicaoDeBiblioteca(this.biblioteca, super.resultado, super.introspector);

  Future<Declaration> _topo(Identifier id) async =>
      _com(await introspector.topLevelDeclarationsOf(biblioteca), id, 'declaração');

  @override
  Future<FunctionDefinitionBuilder> buildFunction(Identifier identifier) async =>
      ConstrutorDeDefinicaoDeFuncao(await _topo(identifier) as FuncaoImpl, resultado, introspector);

  @override
  Future<TypeDefinitionBuilder> buildType(Identifier identifier) async =>
      ConstrutorDeDefinicaoDeTipo(await _topo(identifier) as TypeDeclaration, resultado, introspector);

  @override
  Future<VariableDefinitionBuilder> buildVariable(Identifier identifier) async =>
      ConstrutorDeDefinicaoDeVariavel(await _topo(identifier) as VariableDeclaration, resultado, introspector);
}

class ConstrutorDeDefinicaoDeValor extends _BaseDeDefinicoes implements EnumValueDefinitionBuilder {
  final ValorDeEnumImpl declaracao;
  ConstrutorDeDefinicaoDeValor(this.declaracao, super.resultado, super.introspector);

  @override
  void augment(DeclarationCode entry) => (resultado.valoresDeEnum[declaracao.definingEnum] ??= []).add(entry);
}

class ConstrutorDeDefinicaoDeFuncao extends _BaseDeDefinicoes implements FunctionDefinitionBuilder {
  final FuncaoImpl declaracao;
  ConstrutorDeDefinicaoDeFuncao(this.declaracao, super.resultado, super.introspector);

  @override
  void augment(FunctionBodyCode body, {CommentCode? docComments}) {
    final codigo = augmentacaoDeFuncao(declaracao, body: body, docComments: docComments);
    final d = declaracao;
    if (d is MetodoImpl) {
      (resultado.tipos[d.definingType] ??= []).add(codigo);
    } else {
      resultado.biblioteca.add(codigo);
    }
  }
}

class ConstrutorDeDefinicaoDeConstrutor extends _BaseDeDefinicoes implements ConstructorDefinitionBuilder {
  final ConstrutorImpl declaracao;
  ConstrutorDeDefinicaoDeConstrutor(this.declaracao, super.resultado, super.introspector);

  @override
  void augment({FunctionBodyCode? body, List<Code>? initializers, CommentCode? docComments}) {
    final codigo =
        augmentacaoDeFuncao(declaracao, body: body, initializers: initializers, docComments: docComments);
    (resultado.tipos[declaracao.definingType] ??= []).add(codigo);
  }
}

class ConstrutorDeDefinicaoDeVariavel extends _BaseDeDefinicoes implements VariableDefinitionBuilder {
  final VariableDeclaration declaracao;
  ConstrutorDeDefinicaoDeVariavel(this.declaracao, super.resultado, super.introspector);

  @override
  void augment(
      {DeclarationCode? getter,
      DeclarationCode? setter,
      ExpressionCode? initializer,
      CommentCode? initializerDocComments}) {
    final codigos = augmentacoesDeVariavel(declaracao,
        getter: getter, setter: setter, initializer: initializer, initializerDocComments: initializerDocComments);
    final d = declaracao;
    if (d is CampoImpl) {
      (resultado.tipos[d.definingType] ??= []).addAll(codigos);
    } else {
      resultado.biblioteca.addAll(codigos);
    }
  }
}

// ------------------------------------------------------ texto dos `augment`

/// A declaração `augment` que completa uma variável ou um campo: um
/// `augment` por getter, setter e inicializador pedidos, nessa ordem. Campos
/// levam dois espaços de recuo (o texto vai dentro do corpo da classe).
List<DeclarationCode> augmentacoesDeVariavel(VariableDeclaration d,
    {DeclarationCode? getter,
    DeclarationCode? setter,
    ExpressionCode? initializer,
    CommentCode? initializerDocComments}) {
  if (initializerDocComments != null && initializer == null) {
    throw ArgumentError('initializerDocComments cannot be provided if an initializer is not provided.');
  }
  final campo = d is FieldDeclaration;
  final estatico = d is FieldDeclaration && d.hasStatic;
  List<Object> cabeca() => [if (campo) '  ', 'augment ', if (estatico) 'static '];
  return [
    if (getter != null) DeclarationCode.fromParts([...cabeca(), getter]),
    if (setter != null) DeclarationCode.fromParts([...cabeca(), setter]),
    if (initializer != null)
      DeclarationCode.fromParts([
        if (initializerDocComments != null) initializerDocComments,
        ...cabeca(),
        if (d.hasFinal) 'final ',
        d.type.code,
        ' ',
        d.identifier.name,
        ' = ',
        initializer,
        ';',
      ]),
  ];
}

/// A declaração `augment` que completa uma função, um método ou um
/// construtor: a assinatura repetida (parâmetros como a introspecção os vê,
/// cada um seguido de `, `), a lista de inicialização quebrada em linhas e o
/// corpo (ou `;`). É o texto do CFE 3.6.2, byte a byte.
DeclarationCode augmentacaoDeFuncao(FunctionDeclaration d,
    {FunctionBodyCode? body, List<Code>? initializers, CommentCode? docComments}) {
  assert(initializers == null || d is ConstructorDeclaration);
  final posicionais = d.positionalParameters.toList();
  final obrigatorios = posicionais.takeWhile((p) => p.isRequired);
  final opcionais = posicionais.where((p) => !p.isRequired).toList();
  final tparams = d.typeParameters.toList();
  return DeclarationCode.fromParts([
    if (docComments != null) ...[docComments, '\n'],
    if (d is MethodDeclaration) '  ',
    'augment ',
    if (d is ConstructorDeclaration) ...[
      if (d.isConst) 'const ',
      if (d.isFactory) 'factory ',
      d.definingType.name,
      if (d.identifier.name.isNotEmpty) '.',
    ] else ...[
      if (d is MethodDeclaration && d.hasStatic) 'static ',
      d.returnType.code,
      ' ',
      if (d.isOperator) 'operator ',
    ],
    if (d.isGetter) 'get ',
    if (d.isSetter) 'set ',
    d.identifier.name,
    if (!d.isGetter) ...[
      if (tparams.isNotEmpty) ...[
        '<',
        for (final (i, t) in tparams.indexed) ...[
          t.identifier.name,
          if (t.bound != null) ...[' extends ', t.bound!.code],
          if (i < tparams.length - 1) ', ',
        ],
        '>',
      ],
      '(',
      for (final p in obrigatorios) ...[p.code, ', '],
      if (opcionais.isNotEmpty) ...['[', for (final p in opcionais) ...[p.code, ', '], ']'],
      if (d.namedParameters.isNotEmpty) ...['{', for (final p in d.namedParameters) ...[p.code, ', '], '}'],
      ')',
    ],
    if (initializers != null && initializers.isNotEmpty) ...[
      '\n      : ',
      initializers.first,
      for (final i in initializers.skip(1)) ...[',\n        ', i],
    ],
    if (body == null) ';' else ...[' ', body],
  ]);
}

// --------------------------------------------------------------- introspector

/// O introspector de uma execução: responde do [Modelo] (pré-busca) ou pelo
/// hospedeiro. Serve às três fases; o hospedeiro recusa o que a fase não
/// permite.
final class Introspector implements DefinitionPhaseIntrospector {
  final Modelo modelo;
  Introspector(this.modelo);

  IdentificadorImpl _id(Object d) => switch (d) {
        DeclaracaoImpl() => d.identifier,
        IdentificadorImpl() => d,
        _ => throw ExcecaoDeImplementacao('declaração que não veio do hospedeiro: $d'),
      };

  @override
  Future<Identifier> resolveIdentifier(Uri library, String name) async {
    final r = await modelo.consultar('resolverIdentificador', {'uri': '$library', 'nome': name});
    return modelo.identificador(r as Map<String, Object?>);
  }

  @override
  Future<StaticType> resolve(TypeAnnotationCode type) async {
    final r = await modelo.consultar('resolver', {'tipo': tipoCodigoParaJson(type)});
    return modelo.tipoEstatico(r as Map<String, Object?>);
  }

  @override
  Future<List<EnumValueDeclaration>> valuesOf(EnumDeclaration enuum) async =>
      (await modelo.membros(_id(enuum), 'valores')).cast();

  @override
  Future<List<FieldDeclaration>> fieldsOf(TypeDeclaration type) async =>
      (await modelo.membros(_id(type), 'campos')).cast();

  @override
  Future<List<MethodDeclaration>> methodsOf(TypeDeclaration type) async =>
      (await modelo.membros(_id(type), 'metodos')).cast();

  @override
  Future<List<ConstructorDeclaration>> constructorsOf(TypeDeclaration type) async =>
      (await modelo.membros(_id(type), 'construtores')).cast();

  @override
  Future<List<TypeDeclaration>> typesOf(Library library) async {
    final r = await modelo.consultar('tiposDe', {'biblioteca': (library as BibliotecaImpl).id}) as List;
    return [for (final d in r) modelo.declaracao(d as Map<String, Object?>) as TypeDeclaration];
  }

  @override
  Future<List<Declaration>> topLevelDeclarationsOf(Library library) async {
    final r = await modelo.consultar('declaracoesDe', {'biblioteca': (library as BibliotecaImpl).id}) as List;
    return [for (final d in r) modelo.declaracao(d as Map<String, Object?>)];
  }

  @override
  Future<TypeDeclaration> typeDeclarationOf(Identifier identifier) async {
    final d = await declarationOf(identifier);
    if (d is TypeDeclaration) return d;
    throw ExcecaoDeImplementacao('${identifier.name} não é uma declaração de tipo');
  }

  @override
  Future<Declaration> declarationOf(Identifier identifier) async {
    final r = await modelo.consultar('declaracao', {'ident': _id(identifier).id});
    return modelo.declaracao(r as Map<String, Object?>);
  }

  @override
  Future<TypeAnnotation> inferType(OmittedTypeAnnotation omittedType) async {
    final r = await modelo.consultar('inferirTipo', {'chave': (omittedType as TipoOmitidoImpl).chave});
    return modelo.tipo(r as Map<String, Object?>);
  }
}
