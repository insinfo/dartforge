function custom(n) {
let list = new L$fresh_list_append.Custom.new();
for (let i = 0; i < n; (i = i + 1)) {
  {
    list.add(new L$fresh_list_append.Item.new(i));
  }
}
return list;
};
