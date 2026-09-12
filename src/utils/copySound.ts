import { invoke } from "@tauri-apps/api/core";
import { getMessageApi } from "@/utils/feedback";

export async function playCopySoundFx(): Promise<void> {
  try {
    await invoke("play_copy_sound");
  } catch (rustError) {
    getMessageApi().error(
      rustError instanceof Error ? rustError.message : String(rustError),
    );
  }
}
