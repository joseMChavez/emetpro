import type { SourcePanel } from "$lib/core/panels";
import StudioPanel from "./StudioPanel.svelte";

export const studioPanel: SourcePanel = {
  id: "studio",
  label: "nav.studio",
  icon: "M9 18V5l12-2v13M9 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0ZM21 16a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
  component: StudioPanel,
};
