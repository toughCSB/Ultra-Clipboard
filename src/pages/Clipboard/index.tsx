import { useClipboardWindowEditableFocus } from "@/hooks/useClipboardWindowEditableFocus";
import Footer from "./components/Footer";
import Group from "./components/Group";
import Header from "./components/Header";
import List from "./components/List";

const Clipboard = () => {
  useClipboardWindowEditableFocus();

  return (
    <div
      className="flex h-screen w-full flex-col overflow-hidden rounded-3 bg-slate-100 dark:bg-neutral-900"
      data-tauri-drag-region
    >
      <Header />

      <Group />

      <List />

      <Footer />
    </div>
  );
};

export default Clipboard;
