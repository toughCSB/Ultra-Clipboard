import type { FC, ReactNode } from "react";

interface HighlightProps {
  text: string;

  keyword?: string;

  className?: string;
}

const Highlight: FC<HighlightProps> = (props) => {
  const { text, keyword, className } = props;

  const kw = keyword?.trim() ?? "";

  if (!kw) return <span className={className}>{text}</span>;

  const lowerText = text.toLowerCase();
  const lowerKw = kw.toLowerCase();
  const kwLen = lowerKw.length;

  const nodes: ReactNode[] = [];
  let cursor = 0;
  let matchIndex = lowerText.indexOf(lowerKw);
  let key = 0;

  while (matchIndex !== -1) {
    if (matchIndex > cursor) {
      nodes.push(text.slice(cursor, matchIndex));
    }

    nodes.push(
      <mark className="bg-ant-gold-3" key={key++}>
        {text.slice(matchIndex, matchIndex + kwLen)}
      </mark>,
    );

    cursor = matchIndex + kwLen;
    matchIndex = lowerText.indexOf(lowerKw, cursor);
  }

  if (cursor < text.length) {
    nodes.push(text.slice(cursor));
  }

  return <span className={className}>{nodes}</span>;
};

export default Highlight;
