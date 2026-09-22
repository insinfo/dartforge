// dart:collection Queue/ListQueue/DoubleLinkedQueue: addFirst/addLast/removeFirst/removeLast, iteração, length, toList, clear, addAll.
import 'dart:collection';

void main() {
  var q = Queue<int>();
  print(q.isEmpty);
  print(q.length);
  q.add(1);
  q.addLast(2);
  q.addFirst(0);
  print(q);
  print(q.length);
  print(q.first);
  print(q.last);
  print(q.removeFirst());
  print(q.removeLast());
  print(q);
  q.addAll([5, 6, 7]);
  print(q);
  print(q.toList());
  print(q.contains(6));
  print(q.remove(6));
  print(q.remove(99));
  print(q);
  for (var e in q) {
    print('item $e');
  }
  print(q.map((e) => e * 2).toList());
  print(q.where((e) => e > 1).toList());
  print(q.elementAt(1));
  q.clear();
  print(q.isEmpty);
  try {
    q.removeFirst();
  } catch (e) {
    print('lançou ${e is StateError}');
  }
  try {
    q.first;
  } catch (e) {
    print('lançou ${e is StateError}');
  }

  var lq = ListQueue<String>();
  lq.addLast('b');
  lq.addFirst('a');
  lq.addLast('c');
  print(lq);
  print(lq.join(','));
  print(lq.removeFirst());
  print(lq);
  var lq2 = ListQueue.from([3, 1, 2]);
  print(lq2);
  print(lq2.toList()..sort());
  lq2.removeWhere((e) => e == 1);
  print(lq2);
  lq2.retainWhere((e) => e > 2);
  print(lq2);
  var lq3 = ListQueue.of([1, 2, 3, 4, 5]);
  print(lq3.length);
  print(ListQueue.of([9]).single);
  var lq4 = ListQueue<int>(2);
  for (var i = 0; i < 10; i++) {
    lq4.addLast(i);
  }
  print(lq4);
  print(lq4.length);
  while (lq4.length > 3) {
    lq4.removeFirst();
  }
  print(lq4);
  lq4.forEach((e) => print('f$e'));

  var dl = DoubleLinkedQueue<int>();
  dl.addAll([1, 2, 3]);
  dl.addFirst(0);
  dl.addLast(4);
  print(dl);
  print(dl.firstEntry()?.element);
  print(dl.lastEntry()?.element);
  var entry = dl.firstEntry()!.nextEntry()!;
  print(entry.element);
  entry.prepend(10);
  entry.append(20);
  print(dl);
  print(dl.length);
  entry.remove();
  print(dl);
  print(dl.removeFirst());
  print(dl.removeLast());
  print(dl);
  dl.forEachEntry((e) {
    if (e.element == 20) e.remove();
  });
  print(dl);
  print(dl.toList().reversed.toList());
  dl.clear();
  print(dl.isEmpty);
  print(dl.length);

  Queue<int> fila = Queue.from([1, 2, 3]);
  var soma = 0;
  while (fila.isNotEmpty) {
    var x = fila.removeFirst();
    soma += x;
    if (x == 2) fila.addLast(10);
  }
  print(soma);
  print(Queue.of(['x', 'y']).toString());
  print(Queue<int>().toString());
  var pilha = Queue<String>();
  for (var c in 'abc'.split('')) {
    pilha.addLast(c);
  }
  var inv = StringBuffer();
  while (pilha.isNotEmpty) {
    inv.write(pilha.removeLast());
  }
  print(inv);
  print(Queue.from([1, 2, 3]).cast<num>().length);
  print(ListQueue.from(['a', 'b']).toList().indexOf('b'));
  print(Queue<int>.from([1, 2, 3]).any((e) => e.isEven));
  print(Queue<int>.from([1, 2, 3]).fold<int>(0, (a, b) => a + b));
}
