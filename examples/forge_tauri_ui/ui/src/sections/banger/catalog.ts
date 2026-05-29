// Banger catalog — placeholder for the SDF/raymarch primitives that will
// land in INGEN COMPUTE step §19.4 ("Visualisation Directe").
//
// The previous mesh-rendering pipeline (cube/grid geometry, line/mesh
// shaders) was removed when the section migrated from triangles to
// signed distance fields. This file remains to keep the section's
// namespace alive for the upcoming WGSL raymarch shaders.

export const ForgeBangerCatalog = Object.freeze({
  // Reserved for the SDF raymarcher (vertex + fragment shaders, fullscreen
  // quad geometry). Populated in INGEN COMPUTE §19.4.
});

declare global {
  interface Window {
    ForgeBangerCatalog?: typeof ForgeBangerCatalog;
  }
}

window.ForgeBangerCatalog = ForgeBangerCatalog;
