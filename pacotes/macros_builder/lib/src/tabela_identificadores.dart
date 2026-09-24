/// Identificadores estáveis durante as aplicações de macros de uma biblioteca.
/// Espelha a tabela textual de `macros_host::modelo::Tabela`: o mesmo elemento
/// conserva o id entre modelos e consultas, mesmo após reanálise.
library;

import 'package:analyzer/dart/element/element.dart';

final class TabelaIdentificadores {
  final _ids = <String, int>{};
  final _elementos = <int, Element>{};
  final _bibliotecas = <String, int>{};
  final _omitidos = <String, int>{};
  int _proximo = 1;

  int biblioteca(String uri) =>
      _bibliotecas.putIfAbsent(uri, () => _bibliotecas.length + 1);

  int id(Element elemento) {
    final chave = _chave(elemento);
    final existente = _ids[chave];
    if (existente != null) return existente;
    final id = _proximo++;
    _ids[chave] = id;
    _elementos[id] = elemento;
    return id;
  }

  void vincular(Element elemento, int id) {
    final chave = _chave(elemento);
    final anterior = _ids[chave];
    if (anterior != null && anterior != id) {
      throw StateError('id diferente para $chave: $anterior e $id');
    }
    final outro = _elementos[id];
    if (outro != null && _chave(outro) != chave) {
      throw StateError('id $id já pertence a ${_chave(outro)}');
    }
    _ids[chave] = id;
    _elementos[id] = elemento;
    if (id >= _proximo) _proximo = id + 1;
  }

  Element? elemento(int id) => _elementos[id];

  /// Declarações introduzidas por uma augmentation ainda não têm Element no
  /// analyzer 7.3; a chave textual conserva o id entre fases.
  int idGerado(String chave) =>
      _ids.putIfAbsent('gerado:$chave', () => _proximo++);

  int omitido(String chave) =>
      _omitidos.putIfAbsent(chave, () => _omitidos.length + 1);

  String _chave(Element elemento) {
    final uri = elemento.librarySource?.uri;
    final nome = elemento.name;
    if (uri == null || nome == null) {
      throw StateError('elemento sem URI ou nome');
    }
    if (elemento is ClassElement) return 'tipo:$uri#$nome';
    if (elemento is FieldElement) {
      final dono = elemento.enclosingElement3;
      if (dono is! ClassElement) {
        throw UnsupportedError('campo sem classe: $elemento');
      }
      return 'campo:$uri#${dono.name}.$nome';
    }
    throw UnsupportedError('identificador ainda não coberto: $elemento');
  }
}
