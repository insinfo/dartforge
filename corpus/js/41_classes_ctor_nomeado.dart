// Construtores nomeados, redirecionamento : this(...), vários construtores, Cls.new.
class Cor {
  final int r;
  final int g;
  final int b;

  Cor(this.r, this.g, this.b);

  Cor.cinza(int v) : this(v, v, v);

  Cor.preta() : this.cinza(0);

  Cor.branca() : this.cinza(255);

  Cor.deHex(int hex)
      : r = (hex ~/ 65536) % 256,
        g = (hex ~/ 256) % 256,
        b = hex % 256;

  Cor.vermelha() : this(255, 0, 0);

  @override
  String toString() => 'Cor($r, $g, $b)';
}

class Intervalo {
  final int inicio;
  final int fim;

  Intervalo(this.inicio, this.fim);

  Intervalo.vazio() : this(0, 0);

  Intervalo.ate(int fim) : this(0, fim);

  Intervalo.unitario(int v) : this.ate(v + 1);

  Intervalo.copia(Intervalo outro) : this(outro.inicio, outro.fim);

  int get tamanho => fim - inicio;

  @override
  String toString() => '[$inicio, $fim)';
}

class Nome {
  final String texto;
  Nome(this.texto);
  Nome.vazio() : texto = '';
  @override
  String toString() => 'Nome("$texto")';
}

void main() {
  print(Cor(1, 2, 3));
  print(Cor.cinza(128));
  print(Cor.preta());
  print(Cor.branca());
  print(Cor.deHex(0x123456));
  print(Cor.deHex(0xFF00FF));
  print(Cor.vermelha());

  print(Intervalo(2, 5));
  print(Intervalo.vazio());
  print(Intervalo.ate(4));
  print(Intervalo.unitario(7));
  print(Intervalo.copia(Intervalo(9, 12)));
  print(Intervalo.unitario(7).tamanho);
  print(Intervalo.vazio().tamanho);

  final construtor = Nome.new;
  print(construtor('x'));
  final construtorVazio = Nome.vazio;
  print(construtorVazio());

  final nomes = ['a', 'b', 'c'].map(Nome.new).toList();
  print(nomes);
  final cinzas = [0, 100, 200].map(Cor.cinza).toList();
  print(cinzas);

  final fabricas = <Cor Function()>[Cor.preta, Cor.branca, Cor.vermelha];
  for (final f in fabricas) {
    print(f());
  }
}
