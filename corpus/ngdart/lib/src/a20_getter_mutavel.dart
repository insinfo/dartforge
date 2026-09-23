import 'package:ngdart/angular.dart';

/// Getter é **mutável** para o `isImmutable` do oficial: a `variable` de um
/// getter escrito à mão é sintética. `{{titulo}}` vira ligação de texto,
/// `[title]` ganha `checkBinding` e o `*ngFor` sobre um getter também.
@Component(
  selector: 'a20-getter-mutavel',
  templateUrl: 'a20_getter_mutavel.html',
  directives: [coreDirectives],
)
class A20GetterMutavel {
  String get titulo => 'x';
  List<String> get itens => const ['a', 'b'];
}
