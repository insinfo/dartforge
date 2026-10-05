var x = true;
var l = [1];
const a = [if (x) 1];
const b = [...l];
const c = [for (var i = 0; i < 1; i++) i];
const d = {...l};
const e = {1: 2, ...l};
const f = [...1];
const g = [if (1) 1];
const h = [l];
void fn() {
  const m = [for (var i = 0; i < 1; i++) i];
}
