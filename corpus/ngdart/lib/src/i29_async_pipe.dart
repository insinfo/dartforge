import 'package:ngdart/angular.dart';

/// Sonda: pipe `async`.
@Component(
  selector: 'i29-async-pipe',
  templateUrl: 'i29_async_pipe.html',
  directives: [coreDirectives],
  pipes: [AsyncPipe],
)
class I29AsyncPipe {
  Stream<String> fluxo = Stream.value('a');
}
