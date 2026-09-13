import { useSnapshot } from "valtio";

import { windowLifecycleState } from "@/stores/windowLifecycle";

export const useWindowLifecycle = () => {
  return useSnapshot(windowLifecycleState);
};
