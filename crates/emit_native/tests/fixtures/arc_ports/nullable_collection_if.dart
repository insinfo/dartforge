class Buffer {
  final int fd;

  Buffer(this.fd);
}

List<int> descriptors({Buffer? buffer}) => [if (buffer != null) buffer.fd];

List<int> defaultDescriptors() => descriptors();

List<int> runtimeDescriptors(Buffer? buffer) => descriptors(buffer: buffer);

bool observe(bool value) {
  print('observe:$value');
  return value;
}

void main() {
  print(defaultDescriptors());
  print(runtimeDescriptors(Buffer(7)));
  print(runtimeDescriptors(null));
  print(!observe(true));
  print(!observe(false));
}
