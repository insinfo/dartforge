void main() {
  try {
    Function f = (int x) => x;
    f(1, 2);
  } catch (e) {
    print(e is NoSuchMethodError);
    print(e.toString().startsWith('NoSuchMethodError:'));
  }
}
