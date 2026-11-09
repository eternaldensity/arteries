// A tiny explicit store: the view layer owns no hidden reactive graph.
export function createStore(initial) {
  let state = initial;
  const listeners = new Set();
  return {
    get: () => state,
    set(next) {
      if (Object.is(next, state)) return;
      state = next;
      for (const listener of listeners) listener(state);
    },
    update(reduce) {
      this.set(reduce(state));
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(state);
      return () => listeners.delete(listener);
    },
  };
}
