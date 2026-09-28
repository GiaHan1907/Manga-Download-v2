import { ImageOff } from "lucide-react";
import { useState } from "react";
import type { ReaderPage } from "../../../domain/reader";

export type ReaderImageProps = {
  page: ReaderPage;
  fit: "page" | "width";
  zoom: number;
  onPageIndex: number;
};

export default function ReaderImage({ page, fit, zoom, onPageIndex }: ReaderImageProps) {
  const [failed, setFailed] = useState(false);

  if (failed) {
    return (
      <div className="reader-image reader-image--failed" role="img" aria-label={`Không tải được trang ${onPageIndex + 1}`}>
        <ImageOff size={28} aria-hidden="true" />
        <p>Không tải được ảnh trang mẫu {onPageIndex + 1}. Thử trang khác hoặc quay lại chi tiết truyện.</p>
      </div>
    );
  }

  return (
    <img
      className={`reader-image reader-image--${fit}`}
      src={page.src}
      alt={page.alt}
      style={fit === "page" ? { transform: `scale(${zoom})` } : { width: `${100 * zoom}%` }}
      onError={() => setFailed(true)}
      draggable={false}
    />
  );
}
