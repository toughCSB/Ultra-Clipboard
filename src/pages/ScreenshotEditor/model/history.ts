/** Immutable undo history. Unchanged documents share references between steps. */
export interface History<T> {
  past: readonly T[];
  present: T;
  future: readonly T[];
}

export const HISTORY_LIMIT = 200;

export const createHistory = <T>(present: T): History<T> => {
  return { future: [], past: [], present };
};

/** Records `next` as one undo step. Committing the current document is a no-op. */
export const commitHistory = <T>(
  history: History<T>,
  next: T,
  limit = HISTORY_LIMIT,
): History<T> => {
  if (Object.is(next, history.present)) return history;

  const past = [...history.past, history.present];

  return {
    future: [],
    past: past.slice(Math.max(0, past.length - limit)),
    present: next,
  };
};

export const undoHistory = <T>(history: History<T>): History<T> => {
  if (history.past.length === 0) return history;

  const previous = history.past[history.past.length - 1];

  return {
    future: [history.present, ...history.future],
    past: history.past.slice(0, -1),
    present: previous,
  };
};

export const redoHistory = <T>(history: History<T>): History<T> => {
  if (history.future.length === 0) return history;

  const [next, ...future] = history.future;

  return {
    future,
    past: [...history.past, history.present],
    present: next,
  };
};
