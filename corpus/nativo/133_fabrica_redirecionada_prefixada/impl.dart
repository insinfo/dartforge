import 'main.dart' as m;

class Template implements m.Template {
  final String fonte;
  final bool leniente;
  Template.daFonte(this.fonte, {bool? leniente}) : leniente = leniente ?? false;
  Template() : fonte = '(vazio)', leniente = false;

  @override
  String renderizar(Map<String, Object?> valores) {
    var s = fonte;
    valores.forEach((k, v) => s = s.replaceAll('{{$k}}', '$v'));
    return leniente ? '$s (leniente)' : s;
  }
}

class CaixaImpl<T> implements m.Caixa<T> {
  @override
  final T valor;
  CaixaImpl(this.valor);
}
