class Payload {
  final String text;
  Payload(this.text);
}

class Result {
  final Payload image;
  Result(this.image);
}

@pragma('vm:never-inline')
Result produce(bool fail) {
  if (fail) throw StateError('failed');
  return Result(Payload('ok'));
}

Payload decode(bool fail) => produce(fail).image;

@pragma('vm:never-inline')
void attempt(bool fail) {
  Payload? image;
  try {
    image = decode(fail);
  } on Object {
    image = null;
  }
  print(image == null ? 'caught' : image.text);
}

void main() {
  attempt(false);
  attempt(true);
}
