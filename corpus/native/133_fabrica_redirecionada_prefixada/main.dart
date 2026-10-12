// `factory T(…) = p.T.nome;` — fábrica redirecionadora para um construtor
// nomeado de uma classe de mesmo nome importada com prefixo (o `Template`
// do `mustache_template`; docs/NATIVO-PROJETOS-REAIS.md, C21).

import 'impl.dart' as t;

abstract class Template {
  factory Template(String fonte, {bool leniente}) = t.Template.daFonte;
  factory Template.vazio() = t.Template;
  String renderizar(Map<String, Object?> valores);
}

abstract class Caixa<T> {
  factory Caixa(T valor) = t.CaixaImpl<T>;
  T get valor;
}

void main() {
  final a = Template('Olá, {{nome}}!');
  print(a.renderizar({'nome': 'mundo'}));
  final b = Template('[{{x}}]', leniente: true);
  print(b.renderizar({'x': 42}));
  print(Template.vazio().renderizar({}));
  print(a is t.Template);
  final c = Caixa<int>(7);
  print(c.valor);
  print(c is t.CaixaImpl<int>);
  print(c.runtimeType);
}
