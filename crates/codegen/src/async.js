// Scheduler próprio: listeners pendentes retomam na notificação, sem microtask Promise extra.
// Caixas preservam exatamente uma camada de Future em await e retornos aninhados.
class $dartforgeFuture {
  constructor(type) {
    this.type = type; this.state = 0; this.listeners = [];
    $dartforgeTyped(this, ['future', type]);
  }
  listen(success, failure) {
    this.observed = true;
    const listener = () => this.state === 1 ? success(this.box) : failure(this.error);
    if (this.state === 0) this.listeners.push(listener);
    else queueMicrotask(listener);
  }
  settle(value) {
    if (this.state !== 0) return;
    this.state = 1; this.box = {value};
    const listeners = this.listeners; this.listeners = [];
    for (const listener of listeners) listener();
  }
  fail(error) {
    if (this.state !== 0) return;
    this.state = 2; this.error = error;
    const listeners = this.listeners; this.listeners = [];
    if (listeners.length === 0) queueMicrotask(() => { if (!this.observed) throw error; });
    else for (const listener of listeners) listener();
  }
}
function $dartforgeComplete(future, value) {
  if (value === undefined) value = null;
  const type = future.type;
  if (value === future) { future.fail(new TypeError('Future cannot complete with itself')); return; }
  if (value instanceof $dartforgeFuture && (type[0] === 'void' || $dartforgeIs(value, ['future', type]))) {
    // O Future adotado já resolveu seu contrato: não reaplanar seu valor.
    value.listen(box => future.settle(type[0] === 'void' ? null : box.value), error => future.fail(error));
  } else future.settle(type[0] === 'void' ? null : value);
}
function $dartforgeFutureValue(value, type) {
  const future = new $dartforgeFuture(type);
  // Conecta Futures pendentes antes que a microtask de conclusão deles execute.
  if (value instanceof $dartforgeFuture && (type[0] === 'void' || $dartforgeIs(value, ['future', type])))
    $dartforgeComplete(future, value);
  else queueMicrotask(() => $dartforgeComplete(future, value));
  return future;
}
function $dartforgeAsync(body, type) {
  const future = new $dartforgeFuture(type), iterator = body();
  function resume(value, failed) {
    let step;
    try { step = failed ? iterator.throw(value) : iterator.next(value); }
    catch (error) { future.fail(error); return; }
    if (step.done) { $dartforgeComplete(future, step.value); return; }
    const awaited = step.value;
    if (awaited instanceof $dartforgeFuture) awaited.listen(box => resume(box, false), error => resume(error, true));
    else queueMicrotask(() => resume({value: awaited}, false));
  }
  resume(undefined, false);
  return future;
}
class $dartforgeDuration {
  constructor(parts) { this.microseconds = parts.reduce((sum, part) => sum + part[0] * part[1], 0); $dartforgeTyped(this, ['duration']); }
  get $df_inMicroseconds() { return this.microseconds; }
  get $df_inMilliseconds() { return Math.trunc(this.microseconds / 1000) + 0; }
  get $df_inSeconds() { return Math.trunc(this.microseconds / 1000000) + 0; }
  get $df_inMinutes() { return Math.trunc(this.microseconds / 60000000) + 0; }
  get $df_inHours() { return Math.trunc(this.microseconds / 3600000000) + 0; }
  get $df_inDays() { return Math.trunc(this.microseconds / 86400000000) + 0; }
  get $df_isNegative() { return this.microseconds < 0; }
}
class $dartforgeTimer {
  constructor(duration, callback) {
    this.$df_isActive = true; this.$df_tick = 0;
    this.handle = setTimeout(() => { this.$df_isActive = false; this.$df_tick = 1; callback(); }, Math.max(0, duration.$df_inMilliseconds));
    $dartforgeTyped(this, ['timer']);
  }
  $df_cancel() { clearTimeout(this.handle); this.$df_isActive = false; }
}
function $dartforgeDelayed(duration, computation, type) {
  const future = new $dartforgeFuture(type);
  new $dartforgeTimer(duration, () => {
    try { $dartforgeComplete(future, computation === null ? null : computation()); }
    catch (error) { future.fail(error); }
  });
  return future;
}
function $dartforgeScheduleMicrotask(callback) { queueMicrotask(callback); }
