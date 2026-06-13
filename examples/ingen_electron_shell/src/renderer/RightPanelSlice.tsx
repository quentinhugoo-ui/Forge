export function RightPanelSlice({ open }: { open: boolean }) {
  return <aside id="right-panel" className="rightPanel" aria-label="Plan sidebar" aria-hidden={!open} />;
}
