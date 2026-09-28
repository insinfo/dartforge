import 'dart:async';

import 'package:ngdart/angular.dart';

/// Regras tiradas da especificação (docs/espec-ngdart): gancho por
/// interface indireta (`implements X`, `X extends OnInit`), `&nbsp;` nas
/// pontas do texto, `*dir="chave: x"` com entrada `bool` (o `dir` sem valor
/// vale `true`), `initSubscriptions` antes dos `@HostListener` do componente
/// e `<` dentro de `{{ }}`.
abstract class J111Ciclo implements OnInit {}

@Component(
  selector: 'j111-filho',
  template: '<i>f</i>',
)
class J111Filho implements J111Ciclo {
  @override
  void ngOnInit() {}
}

@Directive(selector: '[j111Adiado]')
class J111Adiado {
  J111Adiado(TemplateRef t, ViewContainerRef v) {
    v.createEmbeddedView(t);
  }

  @Input('j111Adiado')
  bool preservar = false;

  @Input()
  set j111AdiadoForcar(bool v) {}
}

@Directive(selector: '[j111Fonte]')
class J111Fonte {
  final _c = StreamController<String>();

  @Output()
  Stream<String> get gatilho => _c.stream;
}

@Component(
  selector: 'j111-usa',
  template: '''
<j111-filho></j111-filho>
<div>&nbsp;</div>
<p> &nbsp;x </p>
<div *j111Adiado="forcar: f">a</div>
<b j111Fonte (gatilho)="ouvir()">{{ n < 3 }}</b>''',
  directives: [J111Filho, J111Adiado, J111Fonte],
)
class J111Usa {
  bool f = false;
  int n = 1;
  void ouvir() {}

  @HostListener('click')
  void clicou() {}
}
