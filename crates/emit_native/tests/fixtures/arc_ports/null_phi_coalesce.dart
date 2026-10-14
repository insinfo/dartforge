class Piece {
  String render() => throw UnsupportedError('missing');
}

class Label {
  final String? data;
  final Piece? piece;
  Label(this.data, this.piece);

  String get text => data ?? piece!.render();
}

@pragma('vm:never-inline')
void show(Label label) {
  try {
    print(label.text);
  } on UnsupportedError {
    print('caught');
  }
}

void main() {
  show(Label('plain', null));
  show(Label(null, Piece()));
}
