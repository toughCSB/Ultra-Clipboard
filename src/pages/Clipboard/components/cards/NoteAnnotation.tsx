import type { FC } from "react";
import { useSnapshot } from "valtio";
import Highlight from "@/components/Highlight";
import { clipboardViewState } from "@/stores/clipboardView";

interface NoteAnnotationProps {
  note: string;
}

/** Render a normalized non-empty note with search highlighting. */
const NoteAnnotation: FC<NoteAnnotationProps> = (props) => {
  const { note } = props;
  const { keyword } = useSnapshot(clipboardViewState);

  return (
    <div className="w-full whitespace-pre-wrap">
      <i
        aria-hidden="true"
        className="i-lucide:notebook-pen mr-0.5 inline-block size-3.5 translate-y-0.5 text-ant-primary"
      />

      <Highlight className="break-words" keyword={keyword} text={note} />
    </div>
  );
};

export default NoteAnnotation;
