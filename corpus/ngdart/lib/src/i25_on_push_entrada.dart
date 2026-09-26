import 'package:ngdart/angular.dart';

/// Sonda: `OnPush` com `@Input`.
@Component(
  selector: 'i25-on-push-entrada',
  templateUrl: 'i25_on_push_entrada.html',
  directives: [coreDirectives],
  changeDetection: ChangeDetectionStrategy.onPush,
)
class I25OnPushEntrada {
  @Input()
  String titulo = '';
}
