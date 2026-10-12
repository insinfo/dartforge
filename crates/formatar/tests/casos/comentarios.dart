// Copyright.

/// Doc da biblioteca.
library comentarios;

import 'a.dart'; // depois do import
/* bloco */ import 'b.dart';

/**
 * Estilo JavaDoc.
 */
class A {
  // comentário no início do corpo

  int x; // fim de linha
  /* antes */ int y;

  void f(/* vazio */) {}
  void g(int a /* depois de a */, int b) {
    // só um comentário
  }

  void h() {
    var lista = [
      // primeiro
      1, 2, // dois
      3,
    ];
    chamada(a, // comentário
        b);
    return; // fim
  }
}
// Comentário entre declarações.

void main() {
//   código comentado rente
  print('x');   // espaços demais no fim   
  /*
   * bloco de várias linhas
   */
  final y = 1;
}
// último
