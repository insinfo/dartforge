function fixed(n) {
let list = core.List.empty(t$R("|h<|Ra>"), {growable: false});
for (let i = 0; i < n; (i = i + 1)) {
  {
    list[$add](new L$fresh_list_append.Item.new(i));
  }
}
return list;
};
