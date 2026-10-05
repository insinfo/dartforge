T id<T>(T x) => x;
const a = identical(id<int>, id<int>);
const int Function(int) f = id;
const b = identical(f, id<int>);
const c = id == id;
