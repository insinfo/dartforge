/// Consultas de tipo usadas pela fase de definições de `package:json`.
/// Cada `resolver` aloca novas chaves de `StaticType`, como o CFE 3.6.2.
library;

import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/nullability_suffix.dart';
import 'package:analyzer/dart/element/type.dart';

import 'resolver_identificadores.dart';
import 'tabela_identificadores.dart';

final class _TipoEstatico {
  final ClassElement elemento;
  final List<_TipoEstatico> args;
  final bool anulavel;
  _TipoEstatico(this.elemento, this.args, this.anulavel);

  bool igual(_TipoEstatico outro) {
    if (elemento.librarySource.uri != outro.elemento.librarySource.uri ||
        elemento.name != outro.elemento.name ||
        anulavel != outro.anulavel ||
        args.length != outro.args.length) return false;
    for (var i = 0; i < args.length; i++) {
      if (!args[i].igual(outro.args[i])) return false;
    }
    return true;
  }
}

final class ConsultasDefinicoes {
  final ResolvedorIdentificadores resolvedor;
  final Map<int, Map<String, Object?>> membrosPorId;
  TabelaIdentificadores get tabela => resolvedor.tabela;
  final estaticos = <_TipoEstatico>[];

  ConsultasDefinicoes(this.resolvedor,
      [Map<int, Map<String, Object?>>? membrosPorId])
      : membrosPorId = membrosPorId ?? {};

  Future<Object?> consultar(String tipo, Map<String, Object?> args) async =>
      switch (tipo) {
        'resolverIdentificador' => await resolvedor.resolver(
            args['uri'] as String, args['nome'] as String),
        'declaracao' => declaracao(args['ident'] as int),
        'membros' => _membros(args['dono'] as int, args['tipo'] as String),
        'resolver' => _estaticoJson(
            _lerEstatico(Map<String, Object?>.from(args['tipo'] as Map))),
        'ehExatamente' =>
          _estatico(args['a'] as int).igual(_estatico(args['b'] as int)),
        _ => throw UnsupportedError('consulta $tipo ainda não implementada'),
      };

  List<Object?> _membros(int dono, String tipo) {
    final classe = membrosPorId[dono];
    if (classe == null) {
      throw UnsupportedError('membros de $dono ainda não disponíveis');
    }
    final membros = classe[tipo];
    if (membros is! List) {
      throw UnsupportedError('categoria de membros $tipo não coberta');
    }
    return List<Object?>.from(membros);
  }

  _TipoEstatico _lerEstatico(Map<String, Object?> j) {
    if (j['t'] != 'nomeado') {
      throw UnsupportedError('StaticType ainda não nomeado: ${j['t']}');
    }
    final id = (j['ident'] as Map)['id'] as int;
    final elemento = tabela.elemento(id);
    if (elemento is! ClassElement) {
      throw StateError('tipo estático $id desconhecido');
    }
    return _TipoEstatico(
      elemento,
      [
        for (final arg in j['args'] as List)
          _lerEstatico(Map<String, Object?>.from(arg as Map))
      ],
      j['anulavel'] == true,
    );
  }

  _TipoEstatico _estatico(int chave) {
    if (chave < 1 || chave > estaticos.length) {
      throw StateError('tipo estático $chave desconhecido');
    }
    return estaticos[chave - 1];
  }

  Map<String, Object?> _estaticoJson(_TipoEstatico tipo) {
    final declaracaoDoTipo = _declaracaoDaClasse(tipo.elemento);
    final args = [for (final arg in tipo.args) _estaticoJson(arg)];
    estaticos.add(tipo);
    return {
      'chave': estaticos.length,
      'declaracao': declaracaoDoTipo,
      'args': args,
    };
  }

  Map<String, Object?> declaracao(int id) {
    final elemento = tabela.elemento(id);
    if (elemento is! ClassElement) {
      throw UnsupportedError('declaração $id ainda não coberta');
    }
    return _declaracaoDaClasse(elemento);
  }

  Map<String, Object?> _declaracaoDaClasse(ClassElement classe) {
    final uri = classe.librarySource.uri.toString();
    final versao = classe.library.languageVersion.effective;
    final lib = <String, Object?>{
      'k': 'biblioteca',
      'id': tabela.biblioteca(uri),
      'uri': uri,
      'versao': [versao.major, versao.minor],
    };
    final ident = {'id': tabela.id(classe), 'nome': classe.name};
    final tparams = <Map<String, Object?>>[];
    for (final parametro in classe.typeParameters) {
      if (parametro.bound != null) {
        throw UnsupportedError('limite de parâmetro de tipo ainda não coberto');
      }
      tparams.add({
        'k': 'tparam',
        'ident': {
          'id': tabela.idGerado('tparam:$uri#${classe.name}.${parametro.name}'),
          'nome': parametro.name,
        },
        'lib': lib,
        'limite': null,
      });
    }
    final supertipo = classe.supertype;
    final superclasse = supertipo == null ||
            (supertipo.element.name == 'Object' &&
                supertipo.element.librarySource.uri.toString() == 'dart:core')
        ? null
        : _tipoDoAnalyzer(supertipo);
    return {
      'k': 'classe',
      'ident': ident,
      'lib': lib,
      'tparams': tparams,
      'abstract': classe.isAbstract,
      'base': classe.isBase,
      'external': false,
      'final': classe.isFinal,
      'interface': classe.isInterface,
      'mixin': classe.isMixinClass,
      'sealed': classe.isSealed,
      'superclasse': superclasse,
      'interfaces': [for (final t in classe.interfaces) _tipoDoAnalyzer(t)],
      'mixins': [for (final t in classe.mixins) _tipoDoAnalyzer(t)],
    };
  }

  Map<String, Object?> _tipoDoAnalyzer(DartType tipo) {
    if (tipo is TypeParameterType) {
      final parametro = tipo.element;
      final dono = parametro.enclosingElement3;
      if (dono is! ClassElement) {
        throw UnsupportedError('parâmetro de tipo sem classe: $parametro');
      }
      final uri = dono.librarySource.uri.toString();
      return {
        't': 'nomeado',
        'ident': {
          'id': tabela.idGerado('tparam:$uri#${dono.name}.${parametro.name}'),
          'nome': parametro.name,
        },
        'args': <Object?>[],
        if (tipo.nullabilitySuffix == NullabilitySuffix.question)
          'anulavel': true,
      };
    }
    if (tipo is! InterfaceType) {
      throw UnsupportedError('tipo de declaração ainda não coberto: $tipo');
    }
    return {
      't': 'nomeado',
      'ident': {'id': tabela.id(tipo.element), 'nome': tipo.element.name},
      'args': [for (final arg in tipo.typeArguments) _tipoDoAnalyzer(arg)],
      if (tipo.nullabilitySuffix == NullabilitySuffix.question)
        'anulavel': true,
    };
  }
}
