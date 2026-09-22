// Classes básicas: campos, construtor this.x, métodos, getters, toString, defaults.
class Ponto {
  final int x;
  final int y;
  Ponto(this.x, this.y);

  int get somaCoordenadas => x + y;

  Ponto transladar(int dx, int dy) => Ponto(x + dx, y + dy);

  @override
  String toString() => 'Ponto($x, $y)';
}

class Contador {
  int valor = 0;
  final String nome;
  var passo = 1;
  List<int> historico = [];
  String etiqueta = 'contador-' + 'padrao';

  Contador(this.nome, [this.passo = 1]);

  void incrementa() {
    this.valor = this.valor + this.passo;
    historico.add(valor);
  }

  void reseta() {
    valor = 0;
    historico.clear();
  }

  String descreve() {
    return '$nome=$valor (passo $passo)';
  }

  @override
  String toString() => 'Contador($nome, $valor)';
}

class Pessoa {
  String nome;
  int idade;
  bool ativo = true;
  int? apelidoTamanho;

  Pessoa(this.nome, this.idade);

  bool get maiorDeIdade => idade >= 18;

  String saudacao() => 'Olá, $nome!';

  void envelhece() {
    idade++;
  }
}

void main() {
  final p = Ponto(3, 4);
  print(p);
  print(p.x);
  print(p.y);
  print(p.somaCoordenadas);
  print(p.transladar(1, -1));
  print(p.transladar(0, 0).somaCoordenadas);

  final c = Contador('a');
  print(c);
  c.incrementa();
  c.incrementa();
  print(c.descreve());
  print(c.historico);
  print(c.etiqueta);

  final c2 = Contador('b', 5);
  c2.incrementa();
  c2.incrementa();
  c2.incrementa();
  print(c2.descreve());
  print(c2.historico);
  c2.reseta();
  print(c2);
  print(c2.historico.isEmpty);

  final pessoa = Pessoa('Ana', 17);
  print(pessoa.saudacao());
  print(pessoa.maiorDeIdade);
  pessoa.envelhece();
  print(pessoa.idade);
  print(pessoa.maiorDeIdade);
  print(pessoa.ativo);
  print(pessoa.apelidoTamanho);
  pessoa.apelidoTamanho = pessoa.nome.length;
  print(pessoa.apelidoTamanho);
  pessoa.nome = 'Ana Maria';
  print(pessoa.saudacao());

  final lista = [Ponto(1, 1), Ponto(2, 2)];
  print(lista);
  print(lista.map((q) => q.somaCoordenadas).toList());
}
