import 'package:ngdart/angular.dart';

import 'j92_filho_com_ligacoes_do_elemento.dart';

/// Componente único cujo primeiro campo de elemento é o de um filho (o
/// `icon-affix` do ngcomponents): o `dart:html` entra na tabela nesse campo,
/// antes do `ComponentStyles`.
@Component(
  selector: 'j95-campo-do-elemento-do-filho',
  template: '<j92-filho j92-destaque [class.ativo]="ativo"></j92-filho>',
  directives: [J92Filho, J92Destaque],
)
class J95CampoDoElementoDoFilho {
  bool ativo = true;
}
