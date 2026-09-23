// R-FLU-19: o contexto do lado direito de uma atribuição a variável
// promovida é o tipo declarado (não o promovido).
void f(Object o) {
  if (o is List<num>) {
    o = /*@*/[1];
    print(/*@*/o);
  }
}

void main() => f(1);
