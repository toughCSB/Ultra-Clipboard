import type { PixelRect } from "@/utils/pixelRect";

export type OverlaySelectionMode = "area" | "window";

/** The part of the backend overlay state the session model depends on. */
export interface OverlaySnapshot {
  mode: string;
  ready: boolean;
  sessionId: number;
}

export interface OverlaySession {
  committed: boolean;
  generation: number;
  pending: PixelRect | null;
  ready: boolean;
  sessionId: number | null;
}

export type OverlaySessionEvent =
  | { type: "loadStarted" }
  | {
      generation: number;
      snapshot: OverlaySnapshot | null;
      type: "stateLoaded";
    }
  | { rect: PixelRect; type: "commitRequested" }
  | { type: "commitFailed" }
  | { type: "cancelRequested" }
  | { type: "cancelFailed" }
  | { type: "reset" }
  | { type: "unmounted" };

export type OverlaySessionCommand =
  | { rect: PixelRect; sessionId: number; type: "sendCommit" }
  | { sessionId: number; type: "sendCancel" }
  | { mode: OverlaySelectionMode; type: "startSession" }
  | { type: "release" };

export interface OverlaySessionStep {
  commands: OverlaySessionCommand[];
  session: OverlaySession;
}

export const createOverlaySession = (): OverlaySession => {
  return {
    committed: false,
    generation: 0,
    pending: null,
    ready: false,
    sessionId: null,
  };
};

/** Whether a load started at `generation` is still the latest one. */
export const isCurrentLoad = (
  session: OverlaySession,
  generation: number,
): boolean => {
  return session.generation === generation;
};

export const overlaySessionReducer = (
  session: OverlaySession,
  event: OverlaySessionEvent,
): OverlaySessionStep => {
  switch (event.type) {
    case "loadStarted": {
      return step({ ...session, generation: session.generation + 1 });
    }

    case "stateLoaded": {
      if (!isCurrentLoad(session, event.generation)) return step(session);

      return applySnapshot(session, event.snapshot);
    }

    case "commitRequested": {
      if (session.sessionId === null || session.committed) return step(session);

      const committed = { ...session, committed: true };

      // A selection made before the pixels arrive is sent when they do.
      if (!session.ready) return step({ ...committed, pending: event.rect });

      return step(committed, {
        rect: event.rect,
        sessionId: session.sessionId,
        type: "sendCommit",
      });
    }

    case "cancelRequested": {
      if (session.sessionId === null) return step(session);
      if (session.committed && session.pending === null) return step(session);

      return step(
        { ...session, committed: true, pending: null },
        { sessionId: session.sessionId, type: "sendCancel" },
      );
    }

    case "commitFailed":
    case "cancelFailed": {
      return step({ ...session, committed: false });
    }

    case "reset": {
      return resetSession(session);
    }

    case "unmounted": {
      return step({ ...session, generation: session.generation + 1 });
    }
  }
};

const applySnapshot = (
  session: OverlaySession,
  snapshot: OverlaySnapshot | null,
): OverlaySessionStep => {
  if (!snapshot) return resetSession(session);

  const commands: OverlaySessionCommand[] = [];
  let next: OverlaySession = { ...session, ready: snapshot.ready };

  // The pixels of an overlay shown early arrive as a second load of the same
  // session; keep what the user has selected so far.
  if (session.sessionId !== snapshot.sessionId) {
    next = {
      ...next,
      committed: false,
      pending: null,
      sessionId: snapshot.sessionId,
    };
    commands.push({
      mode: snapshot.mode === "window" ? "window" : "area",
      type: "startSession",
    });
  }

  if (snapshot.ready && next.pending) {
    commands.push({
      rect: next.pending,
      sessionId: snapshot.sessionId,
      type: "sendCommit",
    });
    next = { ...next, pending: null };
  }

  return { commands, session: next };
};

const resetSession = (session: OverlaySession): OverlaySessionStep => {
  return step(
    {
      committed: false,
      generation: session.generation + 1,
      pending: null,
      ready: false,
      sessionId: null,
    },
    { type: "release" },
  );
};

const step = (
  session: OverlaySession,
  ...commands: OverlaySessionCommand[]
): OverlaySessionStep => {
  return { commands, session };
};
