sealed class EstadoLogin {}
class Autenticado extends EstadoLogin {}
class NaoAutenticado extends EstadoLogin {}
class Carregando extends EstadoLogin {}

String mapearEstado(EstadoLogin estado) => switch (estado) {
  Autenticado() => 'Bem-vindo de volta!',
  NaoAutenticado() => 'Por favor, faça login.',
  Carregando() => 'Carregando dados...',
};

mixin Voador {
  void voar() => print('Voando alto!');
}
class Passaro with Voador {}

base class Veiculo {
  void mover() => print('Andando...');
}
final class Carro extends Veiculo {}

interface class Pagamento {
  void processar() {}
}
class PagamentoPix implements Pagamento {
  void processar() => print('Pix processado');
}

void main() {
  print(mapearEstado(Autenticado()));
  print(mapearEstado(NaoAutenticado()));
  print(mapearEstado(Carregando()));
  Passaro().voar();
  Carro().mover();
  Pagamento pagamento = PagamentoPix();
  pagamento.processar();
}
