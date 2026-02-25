/** CSS variable names for node kind colors. */
export const KIND_COLORS: Record<string, string> = {
  system: "--kind-system",
  service: "--kind-service",
  component: "--kind-component",
  unit: "--kind-unit",
};

/** Shape names for node sub_kind values. */
export const SUB_KIND_SHAPES: Record<string, string> = {
  trait: "diamond",
  struct: "hexagon",
  function: "ellipse",
  module: "roundrectangle",
  enum: "pentagon",
  interface: "diamond",
  class: "hexagon",
  crate: "roundrectangle",
  package: "roundrectangle",
  workspace: "roundrectangle",
  directory: "roundrectangle",
};

/** Edge visual style per edge kind (excluding 'contains' which is implicit). */
export const EDGE_STYLES: Record<string, { cssVar: string; lineStyle: string; arrowShape: string }> = {
  depends: { cssVar: "--accent", lineStyle: "solid", arrowShape: "triangle" },
  calls: { cssVar: "--kind-component", lineStyle: "solid", arrowShape: "vee" },
  implements: { cssVar: "--pass", lineStyle: "dotted", arrowShape: "triangle" },
  extends: { cssVar: "--kind-service", lineStyle: "solid", arrowShape: "triangle-backcurve" },
  exports: { cssVar: "--kind-system", lineStyle: "dashed", arrowShape: "triangle" },
  data_flow: { cssVar: "--warn", lineStyle: "dashed", arrowShape: "vee" },
};
