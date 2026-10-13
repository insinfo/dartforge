import 'anotacoes.dart';

// Só as declarações de topo contam: a anotação num membro não dispara.
class Membro {
  @Gerar()
  void metodo() {}
}
