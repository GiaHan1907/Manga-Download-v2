import { Component, type ErrorInfo, type ReactNode } from "react";
import ErrorState from "../components/ui/ErrorState";

type ErrorBoundaryProps = {
  children: ReactNode;
};

type ErrorBoundaryState = {
  hasError: boolean;
};

export default class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("UI boundary caught an error", error, info);
  }

  render() {
    if (this.state.hasError) {
      return (
        <ErrorState
          title="Không thể hiển thị giao diện"
          description="Shell đã chặn lỗi để ứng dụng không bị crash. Hãy mở lại trang hoặc kiểm tra log phát triển."
        />
      );
    }

    return this.props.children;
  }
}
