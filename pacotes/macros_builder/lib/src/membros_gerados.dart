/// Lê declarações da augmentation parcial sem depender da associação de
/// `import augment` pelo analyzer 7.3. Só os membros que o builder consegue
/// serializar entram no modelo da fase de definições.
library;

import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/element/element.dart';

import 'tabela_identificadores.dart';

Future<Map<String, Object?>> membrosGerados(
  String fonte,
  ClassElement classe,
  Map<String, Object?> execucao,
  TabelaIdentificadores tabela,
) async {
  final parsed = parseString(
    content: fonte,
    featureSet: FeatureSet.latestLanguageVersion(flags: ['macros']),
    throwIfDiagnostics: false,
  );
  if (parsed.errors.isNotEmpty) {
    throw FormatException('augmentation parcial inválida: ${parsed.errors}');
  }
  final unidade = parsed.unit;
  final imports = <String, String>{};
  for (final directive in unidade.directives.whereType<ImportDirective>()) {
    final prefixo = directive.prefix?.name;
    final uri = directive.uri.stringValue;
    if (prefixo != null && uri != null) imports[prefixo] = uri;
  }
  final declaracoes = unidade.declarations
      .whereType<ClassDeclaration>()
      .where((d) => d.name.lexeme == classe.name)
      .toList();
  if (declaracoes.length != 1) {
    throw StateError('augmentation de ${classe.name} ausente ou duplicada');
  }
  final declaracao = declaracoes.single;
  final alvo = Map<String, Object?>.from(execucao['alvo'] as Map);
  final ident = Map<String, Object?>.from(alvo['ident'] as Map);
  final lib = Map<String, Object?>.from(alvo['lib'] as Map);
  final uriAlvo = lib['uri'] as String;

  Future<Map<String, Object?>> tipo(TypeAnnotation entrada) async {
    if (entrada is! NamedType) {
      throw UnsupportedError('tipo gerado não nomeado: ${entrada.toSource()}');
    }
    final prefixo = entrada.importPrefix?.name.lexeme;
    final uri = prefixo == null ? uriAlvo : imports[prefixo];
    if (uri == null) {
      throw StateError('prefixo $prefixo não importado na augmentation');
    }
    final resultado = await classe.library.session.getLibraryByUri(uri);
    if (resultado is! LibraryElementResult) {
      throw StateError('biblioteca $uri não resolvida pelo analyzer');
    }
    final nome = entrada.name2.lexeme;
    final elementos = resultado.element.topLevelElements
        .whereType<ClassElement>()
        .where((e) => e.name == nome);
    if (elementos.isEmpty) throw StateError('tipo $uri#$nome não encontrado');
    final args = <Map<String, Object?>>[];
    for (final argumento
        in entrada.typeArguments?.arguments ?? const <TypeAnnotation>[]) {
      args.add(await tipo(argumento));
    }
    return {
      't': 'nomeado',
      'ident': {'id': tabela.id(elementos.first), 'nome': nome},
      'args': args,
      if (entrada.question != null) 'anulavel': true,
    };
  }

  Future<Map<String, Object?>> parametro(
      FormalParameter p, String chaveDoDono) async {
    final normal = p is DefaultFormalParameter ? p.parameter : p;
    if (normal is! SimpleFormalParameter ||
        normal.type == null ||
        normal.name == null) {
      throw UnsupportedError('parâmetro gerado não coberto: ${p.toSource()}');
    }
    final nome = normal.name!.lexeme;
    return {
      'k': 'parametro',
      'ident': {
        'id': tabela.idGerado('parametro:$chaveDoDono:$nome'),
        'nome': nome,
      },
      'lib': lib,
      'tipo': await tipo(normal.type!),
      'nomeado': p.isNamed,
      'obrigatorio': p.isRequired,
      'estilo': 'normal',
    };
  }

  final metodos = <Map<String, Object?>>[];
  final construtores = <Map<String, Object?>>[];
  // O hospedeiro serializa métodos antes de construtores, ainda que o AST
  // coloque o construtor primeiro; isso também define a ordem de novos IDs.
  for (final metodo in declaracao.members.whereType<MethodDeclaration>()) {
    if (metodo.returnType == null || metodo.parameters == null) {
      throw UnsupportedError('método gerado sem retorno ou parâmetros');
    }
    final nome = metodo.name.lexeme;
    final chave = '$uriAlvo#${classe.name}.$nome';
    final posicionais = <Map<String, Object?>>[];
    final nomeados = <Map<String, Object?>>[];
    for (final p in metodo.parameters!.parameters) {
      final j = await parametro(p, 'metodo:$chave');
      (p.isNamed ? nomeados : posicionais).add(j);
    }
    metodos.add({
      'k': 'metodo',
      'ident': {'id': tabela.idGerado('metodo:$chave'), 'nome': nome},
      'lib': lib,
      'dono': ident,
      'static': metodo.isStatic,
      'corpo': true,
      'external': metodo.externalKeyword != null,
      'operador': metodo.isOperator,
      'getter': metodo.isGetter,
      'setter': metodo.isSetter,
      'retorno': await tipo(metodo.returnType!),
      'posicionais': posicionais,
      'nomeados': nomeados,
      'tparams': <Object?>[],
    });
  }
  for (final ctor in declaracao.members.whereType<ConstructorDeclaration>()) {
    final nome = ctor.name?.lexeme ?? '';
    final chave = '$uriAlvo#${classe.name}.$nome';
    final posicionais = <Map<String, Object?>>[];
    final nomeados = <Map<String, Object?>>[];
    for (final p in ctor.parameters.parameters) {
      final j = await parametro(p, 'construtor:$chave');
      (p.isNamed ? nomeados : posicionais).add(j);
    }
    construtores.add({
      'k': 'construtor',
      'ident': {'id': tabela.idGerado('construtor:$chave'), 'nome': nome},
      'lib': lib,
      'dono': ident,
      'corpo': true,
      'external': ctor.externalKeyword != null,
      'const': ctor.constKeyword != null,
      'factory': ctor.factoryKeyword != null,
      'retorno': {
        't': 'omitido',
        'chave': tabela.omitido('construtor:$chave:retorno')
      },
      'posicionais': posicionais,
      'nomeados': nomeados,
      'tparams': <Object?>[],
    });
  }
  return {'metodos': metodos, 'construtores': construtores};
}
