import { lazy } from "react";
import { createHashRouter } from "react-router";

const Clipboard = lazy(() => import("@/pages/Clipboard"));
const ContextMenu = lazy(() => import("@/pages/ContextMenu"));
const ContextSubmenu = lazy(async () => {
  const page = await import("@/pages/ContextMenu");
  return { default: page.ContextSubmenu };
});
const Onboarding = lazy(() => import("@/pages/Onboarding"));
const Preference = lazy(() => import("@/pages/Preference"));
const Preview = lazy(() => import("@/pages/Preview"));
const ScreenshotEditor = lazy(() => import("@/pages/ScreenshotEditor"));
const ScreenshotOverlay = lazy(() => import("@/pages/ScreenshotOverlay"));
const ScreenshotPin = lazy(() => import("@/pages/ScreenshotPin"));
const Update = lazy(() => import("@/pages/Update"));

export const router = createHashRouter([
  {
    Component: Clipboard,
    path: "/",
  },
  {
    Component: Preference,
    path: "/preference",
  },
  {
    Component: Onboarding,
    path: "/onboarding",
  },
  {
    Component: ContextMenu,
    path: "/context-menu",
  },
  {
    Component: ContextSubmenu,
    path: "/context-submenu",
  },
  {
    Component: Preview,
    path: "/preview",
  },
  {
    Component: ScreenshotEditor,
    path: "/screenshot-editor",
  },
  {
    Component: ScreenshotOverlay,
    path: "/screenshot-overlay",
  },
  {
    Component: ScreenshotPin,
    path: "/screenshot-pin",
  },
  {
    Component: Update,
    path: "/update",
  },
]);
