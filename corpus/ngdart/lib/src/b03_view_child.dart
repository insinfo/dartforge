import 'dart:html';

import 'package:ngdart/angular.dart';

/// `@ViewChild` por referência de template.
@Component(
  selector: 'b03-view-child',
  templateUrl: 'b03_view_child.html',
)
class B03ViewChild {
  @ViewChild('caixa')
  Element? caixa;
}
