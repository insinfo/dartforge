// finally com return/break/continue: finally roda após avaliar o valor, return no finally sobrescreve, laços.
int valorAvaliado(String tag) {
  print('avaliando $tag');
  return tag.length;
}

int retornaComFinally() {
  try {
    return valorAvaliado('try');
  } finally {
    print('finally depois de avaliar o retorno');
  }
}

int finallySobrescreve() {
  try {
    return 1;
  } finally {
    // ignore: control_flow_in_finally
    return 2;
  }
}

int finallySobrescreveExcecao() {
  try {
    throw 'perdida';
  } finally {
    // ignore: control_flow_in_finally
    return 3;
  }
}

int finallyNaoAlteraValorJaAvaliado() {
  var x = 10;
  try {
    return x;
  } finally {
    x = 99;
    print('x no finally = $x');
  }
}

List<int> finallyMutaObjetoRetornado() {
  final l = [1];
  try {
    return l;
  } finally {
    l.add(2);
  }
}

String retornoNoCatchComFinally() {
  try {
    throw 'e';
  } catch (e) {
    return 'do catch';
  } finally {
    print('finally após return do catch');
  }
}

String retornosAninhados() {
  try {
    try {
      return 'interno';
    } finally {
      print('finally interno');
    }
  } finally {
    print('finally externo');
  }
}

void main() {
  print(retornaComFinally());
  print(finallySobrescreve());
  print(finallySobrescreveExcecao());
  print(finallyNaoAlteraValorJaAvaliado());
  print(finallyMutaObjetoRetornado());
  print(retornoNoCatchComFinally());
  print(retornosAninhados());

  // break dentro de try/finally em laço
  print('--- break ---');
  for (var i = 0; i < 5; i++) {
    try {
      if (i == 2) break;
      print('try $i');
    } finally {
      print('finally $i');
    }
  }

  // continue dentro de try/finally em laço
  print('--- continue ---');
  for (var i = 0; i < 4; i++) {
    try {
      if (i.isOdd) continue;
      print('par $i');
    } finally {
      print('finally $i');
    }
  }

  // continue dentro de catch com finally
  print('--- continue no catch ---');
  var tratados = 0;
  for (var i = 0; i < 4; i++) {
    try {
      if (i != 1) throw i;
      print('sem exceção em $i');
    } catch (e) {
      tratados++;
      continue;
    } finally {
      print('finally $i');
    }
    print('após try $i');
  }
  print('tratados $tratados');

  // break no finally sobrescreve exceção
  print('--- break no finally ---');
  var n = 0;
  while (n < 5) {
    n++;
    try {
      throw 'ignorada';
    } finally {
      print('finally descarta a exceção');
      // ignore: control_flow_in_finally
      break;
    }
  }
  print('n=$n');

  // break rotulado através de finally aninhado
  print('--- break rotulado ---');
  externo:
  for (var i = 0; i < 3; i++) {
    for (var j = 0; j < 3; j++) {
      try {
        if (j == 1) break externo;
        print('$i,$j');
      } finally {
        print('f $i,$j');
      }
    }
  }

  // return dentro de laço dentro de try com finally
  print('--- return em laço ---');
  int busca(List<int> xs) {
    try {
      for (final x in xs) {
        if (x > 2) return x;
      }
      return -1;
    } finally {
      print('finally da busca');
    }
  }

  print(busca([1, 2, 3, 4]));
  print(busca([1]));

  // finally roda mesmo com return em do-while
  print('--- do-while ---');
  int doWhile() {
    var i = 0;
    do {
      try {
        i++;
        if (i == 2) return i * 100;
      } finally {
        print('iteração $i');
      }
    } while (i < 5);
    return 0;
  }

  print(doWhile());

  // exceção no finally após break: propaga
  print('--- exceção no finally após break ---');
  try {
    for (var i = 0; i < 2; i++) {
      try {
        break;
      } finally {
        throw 'do finally $i';
      }
    }
  } catch (e) {
    print(e);
  }
  print('fim');
}
