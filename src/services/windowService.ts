import { getCurrentWindow } from "@tauri-apps/api/window";

async function runWindowCommand(commandName: string, action: () => Promise<unknown>) {
  try {
    await action();
    return true;
  } catch (error) {
    console.error(`Window command failed: ${commandName}`, error);
    return false;
  }
}

export const windowService = {
  minimize() {
    return runWindowCommand("minimize", () => getCurrentWindow().minimize());
  },
  toggleMaximize() {
    return runWindowCommand("toggleMaximize", () => getCurrentWindow().toggleMaximize());
  },
  close() {
    return runWindowCommand("close", () => getCurrentWindow().close());
  },
  async isMaximized() {
    try {
      return await getCurrentWindow().isMaximized();
    } catch (error) {
      console.error("Window command failed: isMaximized", error);
      return false;
    }
  },
};
