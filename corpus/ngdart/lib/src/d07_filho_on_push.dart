import 'package:ngdart/angular.dart';

/// Filho `onPush` com `@Input`.
@Component(
  selector: 'd07-filho-on-push',
  templateUrl: 'd07_filho_on_push.html',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class D07FilhoOnPush {
  @Input()
  String? rotulo = '';
}
