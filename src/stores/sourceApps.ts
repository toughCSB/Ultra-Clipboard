import { proxy } from "valtio";
import { listAllApps } from "@/commands";
import type { ClipboardApp } from "@/types/clipboard";
import { log } from "@/utils/log";

interface SourceAppsState {
  apps: ClipboardApp[];
  loading: boolean;
}

export const sourceAppsState = proxy<SourceAppsState>({
  apps: [],
  loading: false,
});

let sourceAppsPreload: Promise<void> | null = null;
let sourceAppsLoaded = false;
let sourceAppsRequestToken = 0;

/** Preload source-app filter data for the preferences window. */
export function preloadSourceApps() {
  if (sourceAppsLoaded) return Promise.resolve();
  if (sourceAppsPreload) return sourceAppsPreload;

  sourceAppsPreload = runSourceAppsPreload();

  return sourceAppsPreload;
}

/** Refresh filterable apps while preserving metadata for ignored apps. */
export async function refreshSourceApps(preservedIds: string[]) {
  await replaceSourceApps(listAllApps(), "refresh source apps failed", {
    preservedIds,
  });
}

/** Reload all known filterable source apps. */
export async function reloadSourceApps() {
  await loadSourceApps();
}

/** Merge one newly discovered app into both visible lists. */
export function mergeSourceApp(app: ClipboardApp) {
  const merged = new Map(
    sourceAppsState.apps.map((item) => {
      return [item.id, item];
    }),
  );
  merged.set(app.id, app);
  sourceAppsState.apps = Array.from(merged.values());
  sourceAppsLoaded = true;
}

/** Remove deleted source apps from the preferences mirror. */
export function removeSourceApps(ids: string[]) {
  if (ids.length === 0) return;

  const removed = new Set(ids);
  sourceAppsState.apps = sourceAppsState.apps.filter((app) => {
    return !removed.has(app.id);
  });
}

async function loadSourceApps() {
  await replaceSourceApps(listAllApps(), "load source apps failed");
}

async function runSourceAppsPreload() {
  try {
    await loadSourceApps();
  } finally {
    sourceAppsPreload = null;
  }
}

async function replaceSourceApps(
  request: Promise<ClipboardApp[]>,
  errorMessage: string,
  options: { preservedIds?: string[] } = {},
) {
  const token = sourceAppsRequestToken + 1;
  sourceAppsRequestToken = token;
  sourceAppsState.loading = true;

  try {
    const apps = await request;
    if (sourceAppsRequestToken !== token) return;

    sourceAppsState.apps = mergePreservedApps(apps, options.preservedIds ?? []);
    sourceAppsLoaded = true;
  } catch (error) {
    if (sourceAppsRequestToken !== token) return;

    log.warn(errorMessage, error);
  } finally {
    if (sourceAppsRequestToken === token) {
      sourceAppsState.loading = false;
    }
  }
}

function mergePreservedApps(apps: ClipboardApp[], preservedIds: string[]) {
  if (preservedIds.length === 0) return apps;

  const merged = new Map(
    apps.map((app) => {
      return [app.id, app];
    }),
  );
  const preserved = new Set(preservedIds);

  for (const app of sourceAppsState.apps) {
    if (!preserved.has(app.id)) continue;
    if (merged.has(app.id)) continue;

    merged.set(app.id, app);
  }

  return Array.from(merged.values());
}
