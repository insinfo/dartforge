import 'package:ngdart/angular.dart';

import 'i68_content_child_formas.dart';
import 'i68_marca.dart';

/// Sonda: filho com `@ContentChild(.., read:)` recebendo conteúdo — o valor
/// lido é outro token do nó achado (ainda recusado).
@Component(
  selector: 'i73-usa-consulta-read',
  template:
      '<i68-content-child-formas><div i68-marca></div></i68-content-child-formas>',
  directives: [I68ContentChildFormas, I68Marca],
)
class I73UsaConsultaRead {}
