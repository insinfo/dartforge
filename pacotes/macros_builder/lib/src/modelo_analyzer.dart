/// Primeira fatia do adaptador `BuildStep.resolver` -> `macro.executar`:
/// classes simples e seus campos, com o mesmo vocabulário do `dfexec/1`.
/// Os demais tipos de declaração entram quando houver oráculo dirigido.
library;

import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/nullability_suffix.dart';
import 'package:analyzer/dart/element/type.dart';

import 'tabela_identificadores.dart';

Map<String, Object?> modeloDaClasse(ClassElement classe,
    [TabelaIdentificadores? tabelaCompartilhada]) {
  final tabela = tabelaCompartilhada ?? TabelaIdentificadores();
  if (classe.typeParameters.isNotEmpty ||
      classe.interfaces.isNotEmpty ||
      classe.mixins.isNotEmpty ||
      (classe.supertype != null &&
          (classe.supertype!.element.name != 'Object' ||
              classe.supertype!.element.librarySource.uri.toString() !=
                  'dart:core')) ||
      classe.methods.isNotEmpty) {
    throw UnsupportedError(
        'modelo inicial cobre classes sem herança, parâmetros de tipo ou membros executáveis');
  }
  final uri = classe.librarySource.uri.toString();
  final versao = classe.library.languageVersion.effective;
  final biblioteca = <String, Object?>{
    'k': 'biblioteca',
    'id': tabela.biblioteca(uri),
    'uri': uri,
    'versao': [versao.major, versao.minor]
  };
  final identificador = <String, Object?>{
    'id': tabela.id(classe),
    'nome': classe.name
  };

  Map<String, Object?> tipo(DartType entrada) {
    if (entrada is! InterfaceType)
      throw UnsupportedError('tipo não interface: $entrada');
    final elemento = entrada.element;
    final id = tabela.id(elemento);
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
      'ident': {'id': tabela.id(campo), 'nome': nome},
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
  final construtores = <Map<String, Object?>>[];
  for (final ctor in classe.constructors.where((c) => !c.isSynthetic)) {
    if (ctor.isFactory || ctor.isExternal || ctor.parameters.any((p) =>
        !p.isInitializingFormal || p.isNamed || p.hasDefaultValue)) {
      throw UnsupportedError('construtor fora do modelo inicial: $ctor');
    }
    final chave = '$uri#${classe.name}.${ctor.name}';
    final retornoOmitido = tabela.omitido('construtor:$chave:retorno');
    final posicionais = <Map<String, Object?>>[];
    for (final parametro in ctor.parameters) {
      posicionais.add({
        'k': 'parametro',
        'ident': {
          'id': tabela.idGerado('parametro:$chave:${parametro.name}'),
          'nome': parametro.name,
        },
        'lib': biblioteca,
        'tipo': {
          't': 'omitido',
          'chave': tabela.omitido('parametro:$chave:${parametro.name}:tipo'),
        },
        'nomeado': false,
        'obrigatorio': parametro.isRequired,
        'estilo': 'this',
      });
    }
    construtores.add({
      'k': 'construtor',
      'ident': {
        'id': tabela.idGerado('construtor:$chave'),
        'nome': ctor.name,
      },
      'lib': biblioteca,
      'dono': identificador,
      'corpo': true,
      'external': false,
      'const': ctor.isConst,
      'factory': false,
      'retorno': {'t': 'omitido', 'chave': retornoOmitido},
      'posicionais': posicionais,
      'nomeados': <Object?>[],
      'tparams': <Object?>[],
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
        '${identificador['id']}': {
          'campos': campos,
          'construtores': construtores,
          'metodos': <Object?>[]
        },
      },
    },
  };
}
