import type { FC, ReactNode } from "react";

interface ControlFrameProps {
  children: ReactNode;
}

const ControlFrame: FC<ControlFrameProps> = (props) => {
  const { children } = props;

  return <>{children}</>;
};

export default ControlFrame;
