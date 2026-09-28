import 'package:ngdart/angular.dart';

/// `[x]` num filho que nem ele nem uma diretiva do nó recebe é propriedade
/// do elemento (`_visitProperties`), escrita com as ligações do elemento
/// antes das entradas das diretivas (`bindRenderInputs`) — o `[id]` do
/// `tab-button` no `fixed_material_tab_strip`.
@Component(
  selector: 'j135-botao',
  template: '{{rotulo}}',
)
class J135Botao {
  @Input()
  String? rotulo;
}

@Component(
  selector: 'j135-usa',
  template: '<j135-botao [id]="ident" [rotulo]="texto" [title]="texto"></j135-botao>',
  directives: [J135Botao],
)
class J135Usa {
  String ident = 'a';
  String texto = 'b';
}
