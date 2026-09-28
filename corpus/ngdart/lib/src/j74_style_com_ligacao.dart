import 'package:ngdart/angular.dart';

@Directive(selector: '[j74-marca]')
class J74Marca {}

@Component(
  selector: 'j74-filho',
  template: '<i></i>',
)
class J74Filho {}

/// `style="..."` escrito junto de `[style.x]` (o toast do limitless_ui): o
/// atributo no `build()`, a ligação na detecção.
@Component(
  selector: 'j74-style-com-ligacao',
  templateUrl: 'j74_style_com_ligacao.html',
  directives: [J74Filho, J74Marca],
)
class J74StyleComLigacao {
  String topo = '1px';
  int largura = 2;
  bool ativo = true;
  String extra = 'e';
}
