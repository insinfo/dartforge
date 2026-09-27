import 'package:ngdart/angular.dart';

import 'd07_filho_on_push.dart';

/// `@ViewChild` de um filho `onPush` dentro de `*ngIf`.
@Component(
  selector: 'j16-filho-on-push-em-if',
  templateUrl: 'j16_filho_on_push_em_if.html',
  directives: [coreDirectives, D07FilhoOnPush],
)
class J16FilhoOnPushEmIf {
  bool a = true;

  @ViewChild('f')
  D07FilhoOnPush? filho;
}
