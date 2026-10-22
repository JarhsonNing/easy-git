type DragTone = "branch" | "remote" | "commit" | "tag";

export function setDragGhost(event: DragEvent, text: string, tone: DragTone): void {
  const transfer = event.dataTransfer;
  if (!transfer) return;
  const ghost = document.createElement("div");
  ghost.className = `drag-ghost tone-${tone}`;
  const dot = document.createElement("span");
  dot.className = "drag-ghost-dot";
  const label = document.createElement("span");
  label.className = "drag-ghost-label";
  label.textContent = text;
  ghost.append(dot, label);
  document.body.append(ghost);
  transfer.setDragImage(ghost, 14, 16);
  window.setTimeout(() => ghost.remove(), 0);
}
