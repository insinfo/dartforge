// Função local direta que grava, pelo endereço, um local `bool` (ou
// `double`) de quem a declara (`funcoes_diretas.rs`, captura por
// endereço): a gravação tem a largura do local de lá. Gravado como `i64`,
// o `bool` pisava os bytes vizinhos do quadro (o `listenerHasError` do
// `_Future._propagateToListeners` zerava o ponteiro da área de globais, e
// o servidor HTTP caía quando o cliente fechava a conexão). A saída tem de
// ser a da VM.
String varios(int n) {
  var a = n;
  var f1 = false;
  var b = n * 2;
  var f2 = false;
  var d = 0.5;
  var f3 = true;
  var c = n * 3;
  void liga() {
    f1 = true;
    f2 = !f2;
    f3 = false;
    d = d * 3;
  }

  void mexe() {
    a++;
    b--;
    c += 10;
  }

  for (var i = 0; i < n; i++) {
    liga();
    mexe();
  }
  return '$a $f1 $b $f2 $d $f3 $c';
}

class Resultado {
  Object? valor;
  bool erro = false;
}

// O molde do `_propagateToListeners`: o erro e o valor gravados pelas
// funções locais, lidos depois por quem as declarou.
Resultado propagar(Object? fonte, bool falha) {
  var temErro = false;
  Object? valorOuErro;
  var contador = 0;
  void comValor() {
    try {
      if (falha) throw StateError('falhou $fonte');
      valorOuErro = 'valor $fonte';
    } catch (e) {
      valorOuErro = e;
      temErro = true;
    }
    contador++;
  }

  void comErro() {
    temErro = true;
    valorOuErro = 'erro $fonte';
    contador += 2;
  }

  if (fonte is int && fonte.isEven) {
    comErro();
  } else {
    comValor();
  }
  return Resultado()
    ..valor = '$valorOuErro/$contador'
    ..erro = temErro;
}

void main() {
  for (final n in [0, 1, 2, 5]) {
    print(varios(n));
  }
  for (final (f, x) in [(1, false), (2, false), (3, true), ('s', false), (null, true)]) {
    final r = propagar(f, x);
    print('${r.valor} ${r.erro}');
  }
}
