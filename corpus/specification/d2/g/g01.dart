void f(o) async {
  var variable = 0;
  switch (o) {
    case ++variable:
  }
  switch (o) {
    case const ++variable:
  }
  switch (o) {
    case assert(false):
  }
  switch (o) {
    case switch (o) { _ => true }:
  }
  switch (o) {
    case await 0:
  }
  switch (o) {
    case const void fun() {}:
  }
  switch (o) {
    case const assert(false):
  }
  switch (o) {
    case -variable:
    case !false:
    case const !false:
    case variable--:
  }
}
