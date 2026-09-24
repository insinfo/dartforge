/// Consulta `resolverIdentificador` do hospedeiro de macros, atendida pela
/// sessão do analyzer que o `BuildStep.resolver` já usa.
library;

import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/type.dart';

final class ResolvedorIdentificadores {
  final ClassElement classe;
  final ids = <String, int>{};
  int _proximo = 1;

  ResolvedorIdentificadores(this.classe, Map<String, Object?> execucao) {
    final alvo = Map<String, Object?>.from(execucao['alvo'] as Map);
    final ident = Map<String, Object?>.from(alvo['ident'] as Map);
    _registrar(classe, ident['id'] as int);
    final modelo = Map<String, Object?>.from(execucao['modelo'] as Map);
    final membros = Map<String, Object?>.from(modelo['membros'] as Map);
    final campos =
        Map<String, Object?>.from(membros['1'] as Map)['campos'] as List;
    final elementos =
        classe.fields.where((campo) => !campo.isSynthetic).toList();
    if (campos.length != elementos.length)
      throw StateError('campos do modelo e analyzer diferem');
    for (var i = 0; i < campos.length; i++) {
      final campo = Map<String, Object?>.from(campos[i] as Map);
      _registrar(elementos[i], (campo['ident'] as Map)['id'] as int);
      _registrarTipo(
          elementos[i].type, Map<String, Object?>.from(campo['tipo'] as Map));
    }
  }

  void _registrar(Element elemento, int id) {
    ids[_chave(elemento)] = id;
    if (id >= _proximo) _proximo = id + 1;
  }

  void _registrarTipo(DartType tipo, Map<String, Object?> j) {
    if (tipo is! InterfaceType || j['t'] != 'nomeado') {
      throw UnsupportedError('tipo fora do modelo inicial: $tipo');
    }
    _registrar(tipo.element, (j['ident'] as Map)['id'] as int);
    final argumentos = j['args'] as List;
    if (argumentos.length != tipo.typeArguments.length)
      throw StateError('argumentos de tipo diferentes');
    for (var i = 0; i < argumentos.length; i++) {
      _registrarTipo(tipo.typeArguments[i],
          Map<String, Object?>.from(argumentos[i] as Map));
    }
  }

  Future<Map<String, Object?>> resolver(String uri, String nome) async {
    final resultado = await classe.library.session.getLibraryByUri(uri);
    if (resultado is! LibraryElementResult)
      throw StateError('biblioteca $uri não resolvida pelo analyzer');
    // O CFE da 1ª geração procura declarações da própria biblioteca;
    // reexportações não participam de `resolveIdentifier`.
    final locais =
        resultado.element.topLevelElements.where((e) => e.name == nome);
    final elemento = locais.isEmpty ? null : locais.first;
    if (elemento == null) throw StateError('$uri não exporta $nome');
    final chave = _chave(elemento);
    final id = ids.putIfAbsent(chave, () => _proximo++);
    return {'id': id, 'nome': elemento.name};
  }

  String _chave(Element elemento) {
    final uri = elemento.librarySource?.uri;
    final nome = elemento.name;
    if (uri == null || nome == null)
      throw StateError('elemento sem URI ou nome');
    return '$uri#$nome';
  }
}
