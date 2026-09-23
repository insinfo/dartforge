import 'package:ngdart/angular.dart';

import 'd08_filho_injetado.dart';

/// Filho com injeção: `debugInjectorWrap` sob `isDevMode`, o serviço pela
/// visão de cima (`injectFromViewParentInjector`, pela cadeia de
/// `parentView` numa visão embutida), o nó e a visão do filho. Filho com
/// `*ngIf` é a raiz da visão embutida.
@Component(
  selector: 'd08-usa-injetado',
  templateUrl: 'd08_usa_injetado.html',
  directives: [coreDirectives, D08FilhoInjetado],
)
class D08UsaInjetado {
  bool mostra = true;
}
