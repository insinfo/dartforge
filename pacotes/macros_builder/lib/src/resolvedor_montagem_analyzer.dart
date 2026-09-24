/// Resolução da montagem para classes simples já registradas na tabela do
/// builder. A fase de definições acrescentará membros e tipos inferidos.
library;

import 'package:analyzer/dart/element/element.dart';
import 'package:macros/src/executor/montagem.dart';

import 'tabela_identificadores.dart';

final class ResolvedorMontagemAnalyzer implements ResolvedorMontagem {
  final TabelaIdentificadores tabela;
  ResolvedorMontagemAnalyzer(this.tabela);

  @override
  IdentificadorResolvido identificador(int id) {
    final elemento = tabela.elemento(id);
    if (elemento is ClassElement) {
      return IdentificadorResolvido(elemento.name, TipoDeIdentificador.topo,
          uri: elemento.librarySource.uri.toString());
    }
    if (elemento is FieldElement) {
      final dono = elemento.enclosingElement3;
      if (dono is! ClassElement) {
        throw UnsupportedError('campo sem classe: $elemento');
      }
      final uri = elemento.librarySource?.uri.toString();
      if (elemento.isStatic && uri == null) {
        throw StateError('campo estático sem biblioteca: $elemento');
      }
      return IdentificadorResolvido(
        elemento.name,
        elemento.isStatic
            ? TipoDeIdentificador.estatico
            : TipoDeIdentificador.instancia,
        uri: elemento.isStatic ? uri : null,
        escopo: elemento.isStatic ? dono.name : null,
      );
    }
    throw UnsupportedError('identificador $id ainda não montável');
  }

  @override
  TipoAumentado tipoAumentado(int id) {
    final elemento = tabela.elemento(id);
    if (elemento is! ClassElement || elemento.typeParameters.isNotEmpty) {
      throw UnsupportedError('tipo $id ainda não montável');
    }
    return TipoAumentado(
        'class',
        [
          if (elemento.isAbstract) 'abstract',
          if (elemento.isBase) 'base',
          if (elemento.isFinal) 'final',
          if (elemento.isInterface) 'interface',
          if (elemento.isMixinClass) 'mixin',
          if (elemento.isSealed) 'sealed',
        ],
        elemento.name);
  }

  @override
  Map<String, Object?>? tipoInferido(int chave) => null;
}
