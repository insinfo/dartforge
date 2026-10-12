void main() {
  final resultado = umaFuncaoComNomeBemComprido(primeiroArgumento, segundoArgumento, terceiroArgumento, quartoArgumento);
  final mapa = {'chave um': valorUm, 'chave dois': valorDois, 'chave tres': valorTres, 'chave quatro': valor};
  final condicao = algumaCoisaMuitoLonga && outraCoisaMuitoLonga || maisUmaCoisaMuitoLonga && aindaOutraCoisaLonga;
  final ternario = condicaoMuitoLongaParaCaber ? valorQuandoVerdadeiroMuitoLongo : valorQuandoFalsoMuitoLongo;
  objetoComNomeLongo.primeiroMetodo(argumento).segundoMetodo().terceiroMetodo(outroArgumento, maisUm);
  final widget = Container(child: Column(children: [Text('olá mundo'), Text('outro texto aqui'), Icon(Icons.add)]));
  lista.where((elemento) => elemento.propriedade > 10).map((elemento) => elemento.valor * 2).toList();
  final string = 'uma string muito longa que passa da largura da página ' 'e continua aqui adjacente';
  algumaFuncao(argumento, () {
    print('corpo do callback');
  }, depois: 1);
  final builder = StringBuffer()..write('a')..write('b')..writeln();
  if (primeiraCondicaoMuitoLonga && segundaCondicaoMuitoLonga && terceiraCondicao) {
    print('sim');
  } else if (outra) {
    print('não');
  } else {}
  return switch (valor) { Forma.circulo => 'círculo com nome longo', Forma.quadrado => 'quadrado com nome longo', _ => 'outra forma qualquer' };
}
class Classe extends SuperClasseComNomeMuitoLongo implements InterfaceUm, InterfaceDois, InterfaceTres {
  Classe({required this.campoUm, required this.campoDois, this.campoTres = 3, super.chave}) : assert(campoUm != null);
  Future<Map<String, List<int>>> metodoGenerico<T extends Comparable<T>>(T valor, {int limite = 10}) async => {};
}
