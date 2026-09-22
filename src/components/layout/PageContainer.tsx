import type { ReactNode } from "react";

export type PageContainerProps = {
  children: ReactNode;
};

export default function PageContainer({ children }: PageContainerProps) {
  return (
    <main className="page-scroll" id="main-content" tabIndex={-1}>
      <div className="page-container">{children}</div>
    </main>
  );
}
