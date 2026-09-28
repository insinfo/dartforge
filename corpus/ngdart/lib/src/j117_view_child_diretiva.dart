import 'package:ngdart/angular.dart';

/// `@ViewChild(Diretiva)`: o valor é o campo da diretiva no elemento que ela
/// casa (`_providers.get(tipo).build()`), com o `.instance` de uma
/// `XNgCd`, atribuído no fim do `build()` (o `FocusContentWrapper` do
/// `focus_trap`, o `ButtonDirective` do `dropdown_button`).
@Directive(selector: '[j117Envoltorio]')
class J117Envoltorio {}

@Directive(selector: '[j117Botao]')
class J117Botao {
  @HostBinding('attr.role')
  String papel = 'button';
}

@Component(
  selector: 'j117-view-child-diretiva',
  template: '''
<div j117Envoltorio (click)="clicou()"><span j117Botao>b</span></div>''',
  directives: [J117Envoltorio, J117Botao],
)
class J117ViewChildDiretiva {
  @ViewChild(J117Envoltorio)
  J117Envoltorio? envoltorio;

  @ViewChild(J117Botao)
  set botao(J117Botao? b) {}

  void clicou() {}
}
