function build(count) {
let t$0, t$1, t$2, t$3, t$4, t$5;
let rows = (t$1 = _interceptors.JSArray, t$2 = t$1.of, t$0 = _interceptors.t$freshListAppend470.create(t$2, t$1, t$R("|F<|Ra>"), []), t$0.list);
for (let i = 0; i < count; (i = i + 1)) {
  {
    (t$3 = rows, t$4 = _interceptors.t$freshListAppend470.ready(t$0, t$3), t$5 = t$3[$add], _interceptors.t$freshListAppend470.append(t$0, t$3, t$5, t$4, new L$field_interface.Row.new(i)));
  }
}
return rows;
};
