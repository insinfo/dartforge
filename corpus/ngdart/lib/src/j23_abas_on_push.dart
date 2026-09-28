import 'package:ngdart/angular.dart';

import 'd07_filho_on_push.dart';

/// Contêiner com `@ContentChildren`/`@ContentChild` de um componente
/// `onPush` (as abas do limitless_ui).
@Component(
  selector: 'j23-abas-on-push',
  template: '<ng-content></ng-content>',
)
class J23AbasOnPush {
  @ContentChildren(D07FilhoOnPush)
  List<D07FilhoOnPush>? abas;

  @ContentChild(D07FilhoOnPush)
  D07FilhoOnPush? primeira;
}
