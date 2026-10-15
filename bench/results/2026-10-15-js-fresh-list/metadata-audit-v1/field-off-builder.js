function build(count) {
let rows = _interceptors.JSArray.of(t$R("|F<|Ra>"), []);
for (let i = 0; i < count; (i = i + 1)) {
  {
    rows[$add](new L$field.Row.new(i));
  }
}
return rows;
};
