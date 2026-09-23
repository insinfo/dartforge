// Tipos a imitar: métodos síncronos e assíncronos, getters, genéricos,
// parâmetros nomeados e opcionais.
abstract class Relogio {
  int agora();
}

class Repositorio<T> {
  Future<T?> buscar(int id) async => null;
  Future<void> salvar(int id, T valor) async {}
  Stream<T> todos() => const Stream.empty();
  int get tamanho => 0;
}

class Servico {
  final Repositorio<String> repositorio;
  final Relogio relogio;
  Servico(this.repositorio, this.relogio);

  Future<String> descrever(int id, {String prefixo = '#', int? limite}) async {
    final valor = await repositorio.buscar(id) ?? '?';
    final texto = '$prefixo$id=$valor@${relogio.agora()}';
    return limite == null || texto.length <= limite ? texto : texto.substring(0, limite);
  }
}
