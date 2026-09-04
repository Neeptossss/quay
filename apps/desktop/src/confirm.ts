export const PUBLIC_COMMANDS = new Set([
  "pr.merge",
  "review.approve",
  "review.request_changes",
  "pr.comment",
  "pr.reply",
  "list.archive",
]);

export function needsConfirmation(command: string): boolean {
  return PUBLIC_COMMANDS.has(command);
}

export class Confirmation {
  private awaiting: string | null = null;

  get pendingCommand(): string | null {
    return this.awaiting;
  }

  clear() {
    this.awaiting = null;
  }

  accept(command: string): boolean {
    if (!needsConfirmation(command)) {
      this.awaiting = null;
      return true;
    }
    if (this.awaiting === command) {
      this.awaiting = null;
      return true;
    }
    this.awaiting = command;
    return false;
  }
}
