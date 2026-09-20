enum StatusPedido {
  pendente(1, 'Aguardando Pagamento'),
  pago(2, 'Pago com Sucesso'),
  enviado(3, 'Produto a Caminho'),
  entregue(4, 'Entregue ao Cliente');

  final int codigo;
  final String descricao;
  const StatusPedido(this.codigo, this.descricao);

  bool get jaFoiPago => codigo >= 2;
  void emitirAlerta() {
    print('O pedido mudou para: ' + descricao);
  }
}

T identidade<T>(T valor) => valor;
String resumo(StatusPedido status) => switch (status) {
  StatusPedido.pendente => 'Pendente',
  StatusPedido.pago => 'Pago',
  StatusPedido.enviado => 'Enviado',
  StatusPedido.entregue => 'Entregue',
};

void main() {
  const etapas = <int>[1, 2, 3, 4];
  var status = identidade(StatusPedido.enviado);
  print(status.descricao);
  print(status.jaFoiPago);
  status.emitirAlerta();
  print(resumo(status));
  print(etapas);
}
