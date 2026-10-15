function createRows(count) {
let rows = _interceptors.JSArray.of(t$R("|F<|Ra>"), []);
for (let i = 0; i < count; (i = i + 1)) {
  {
    rows[$add](new L$fresh_list_append.Row.new(i, (i & 255) >>> 0));
  }
}
return rows;
