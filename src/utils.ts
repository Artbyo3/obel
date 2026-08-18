const div = document.createElement("div");
div.appendChild(document.createTextNode(""));

export function escapeHtml(text: string): string {
  div.textContent = text;
  return div.innerHTML;
}
