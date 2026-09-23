// Poda do perfil de produção (docs/JS-PRODUCAO.md §1.7): estáticos e
// variáveis de topo são preguiçosos, então apagar o que ninguém lê não pode
// mudar o que o programa imprime — e o que é lido tem de continuar com o
// inicializador na mesma ordem. Se a poda errar, a ordem das linhas muda ou
// o caso imprime "caso <nome>: ERRO".

void caso(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('caso $nome: ERRO $e');
  }
}

int registra(String quem, int v) {
  print('  inicializa $quem');
  return v;
}

final lida = registra('lida', 1);
final naoLida = registra('naoLida', 2);
late final tardia = registra('tardia', 3);

class Config {
  static final porta = registra('Config.porta', 8080);
  static final esquecida = registra('Config.esquecida', 0);
  static int contador = 0;
  static const nome = 'app';
}

int funcaoNuncaChamada() {
  print('não devia imprimir');
  return registra('nunca', 9);
}

void main() {
  print('início');
  caso('topo lida', () => lida);
  caso('estático lido', () => Config.porta);
  caso('estático escrito', () {
    Config.contador += 2;
    return Config.contador;
  });
  caso('const', () => Config.nome);
  caso('late', () => tardia + tardia);
  print('fim');
}
