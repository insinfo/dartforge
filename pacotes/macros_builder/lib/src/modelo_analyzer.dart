/// Primeira fatia do adaptador `BuildStep.resolver` -> `macro.executar`:
/// classes simples e seus campos, com o mesmo vocabulário do `dfexec/1`.
/// Os demais tipos de declaração entram quando houver oráculo dirigido.
library;

import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/nullability_suffix.dart';
import 'package:analyzer/dart/element/type.dart';

Map<String, Object?> modeloDaClasse(ClassElement classe) {
  if (classe.typeParameters.isNotEmpty ||
      classe.interfaces.isNotEmpty ||
      classe.mixins.isNotEmpty ||
      (classe.supertype != null &&
          (classe.supertype!.element.name != 'Object' ||
              classe.supertype!.element.librarySource.uri.toString() !=
                  'dart:core')) ||
      classe.constructors.any((c) => !c.isSynthetic) ||
      classe.methods.isNotEmpty) {
    throw UnsupportedError(
        'modelo inicial cobre classes sem herança, parâmetros de tipo ou membros executáveis');
  }
  final uri = classe.librarySource.uri.toString();
  final versao = classe.library.languageVersion.effective;
  final biblioteca = <String, Object?>{
    'k': 'biblioteca',
    'id': 1,
    'uri': uri,
    'versao': [versao.major, versao.minor]
  };
  final identificador = <String, Object?>{'id': 1, 'nome': classe.name};
  var proximoId = 2;
  final tipos = <String, int>{};

  Map<String, Object?> tipo(DartType entrada) {
    if (entrada is! InterfaceType)
      throw UnsupportedError('tipo não interface: $entrada');
    final elemento = entrada.element;
    final chave = '${elemento.librarySource.uri}#${elemento.name}';
    final id = tipos.putIfAbsent(chave, () => proximoId++);
    final saida = <String, Object?>{
      't': 'nomeado',
      'ident': {'id': id, 'nome': elemento.name},
      'args': [for (final argumento in entrada.typeArguments) tipo(argumento)],
    };
    if (entrada.nullabilitySuffix == NullabilitySuffix.question)
      saida['anulavel'] = true;
    return saida;
  }

  final campos = <Map<String, Object?>>[];
  for (final campo in classe.fields.where((c) => !c.isSynthetic)) {
    final nome = campo.name;
    final tipoDoCampo = tipo(campo.type);
    campos.add({
      'k': 'campo',
      'ident': {'id': proximoId++, 'nome': nome},
      'lib': biblioteca,
      'dono': identificador,
      'tipo': tipoDoCampo,
      'abstract': campo.isAbstract,
      'const': campo.isConst,
      'external': campo.isExternal,
      'final': campo.isFinal,
      'inicializador': campo.hasInitializer,
      'late': campo.isLate,
      'static': campo.isStatic,
    });
  }
  return {
    'alvo': {
      'k': 'classe',
      'ident': identificador,
      'lib': biblioteca,
      'tparams': <Object?>[],
      'abstract': classe.isAbstract,
      'base': classe.isBase,
      'external': false,
      'final': classe.isFinal,
      'interface': classe.isInterface,
      'mixin': classe.isMixinClass,
      'sealed': classe.isSealed,
      'superclasse': null,
      'interfaces': <Object?>[],
      'mixins': <Object?>[],
    },
    'modelo': {
      'bibliotecas': [biblioteca],
      'membros': {
        '1': {
          'campos': campos,
          'construtores': <Object?>[],
          'metodos': <Object?>[]
        },
      },
    },
  };
}
