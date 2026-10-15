_interceptors.t$freshListAppend471 = ((owner, addKey, growKey) => {
  const apply = Reflect.apply, descriptor = Object.getOwnPropertyDescriptor;
  const hasOwn = Object.prototype.hasOwnProperty;
  const symbols = Object.getOwnPropertySymbols, prototype = Object.getPrototypeOf;
  const arrayPrototype = Array.prototype;
  const originalFactory = owner.of, originalAdd = owner.prototype[addKey];
  const data = (object, key) => {
    const d = descriptor(object, key);
    return d && apply(hasOwn, d, ['value']) ? d : null;
  };
  const ready = (token, list) => token.valid && token.list === list &&
    prototype(list) === arrayPrototype && !descriptor(list, addKey) &&
    data(arrayPrototype, addKey)?.value === originalAdd;
  return {
    create(factory, receiver, rti, array) {
      const list = apply(factory, receiver, [rti, array]);
      const token = {list, rti, key: null, valid: false};
      if (factory === originalFactory && list === array && prototype(array) === arrayPrototype) {
        const keys = symbols(array);
        if (keys.length === 1 && data(array, keys[0])?.value === rti) {
          token.key = keys[0]; token.valid = true;
        }
      }
      return token;
    },
    ready,
    append(token, list, callee, before, value) {
      if (!before || !ready(token, list) || callee !== originalAdd || data(list, token.key)?.value !== token.rti)
        return apply(callee, list, [value]);
      list[growKey]('add');
      list.push(value);
    }
  };
})(_interceptors.JSArray, $add, $checkGrowable);
