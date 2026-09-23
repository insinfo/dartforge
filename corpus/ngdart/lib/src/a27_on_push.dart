import 'package:ngdart/angular.dart';

/// `changeDetection: ChangeDetectionStrategy.onPush`: a visão começa em
/// `checkOnce` (`_getChangeDetectionCheckMode`), e com `@Input` a hospedeira
/// declara `changed` e marca a checagem (`bindDirectiveInputs`). A raiz da
/// visão embutida com ligação é campo, e o `initRootNode` a lê como
/// `this._el_0`.
@Component(
  selector: 'a27-on-push',
  templateUrl: 'a27_on_push.html',
  directives: [coreDirectives],
  changeDetection: ChangeDetectionStrategy.onPush,
)
class A27OnPush {
  @Input()
  String titulo = '';
  List<String> itens = ['a'];
}
