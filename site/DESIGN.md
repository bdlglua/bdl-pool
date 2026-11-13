# BDL Pool design status

On 13 November 2026 the owner asked for the earlier "pulse" design back: dark
navy surfaces, purple/lime accents, DM Sans, floating glass navigation, tilted
node panel, orbit illustration and illustrated shortcuts. `assets/pulse-theme.css`,
the home templates and the chart colours follow that design again. The
instrument theme described below is kept for reference only and is not live.

# BDL Pool / BDLMiner instrument theme

The shared direction was approved on 11 November 2026: technical, precise,
accessible to hobbyists. The visual world is an electronics bench and its
measuring instruments. Keep the existing orange chip identity and the actual
product; put detail into the hardware photograph and measured data.

## Audit and treatment

The previous first screens used purple/green radial washes, gradient wordmarks,
glowing actions, translucent pictograms and tilted data panels. Heavy headings,
small tracked capitals and irregular padding reduced the hierarchy. These were
visual problems; the product features and the section order were retained.

1. **Spacing.** Controls, panels and sections now follow a 4 px rhythm. Panels
   use 24 px padding, and related links and readings have consistent gaps.
2. **Color.** Graphite surfaces, three text roles and the logo's Bitcoin orange
   replace the purple/lime chrome. The primary dark-theme action is off-white.
   Green/amber remain semantic states. A complete light palette is included.
3. **Icons.** Existing line icons are unified at 18 px / 1.5 stroke. Shortcut
   pictograms and chat emoji become restrained line icons or text.
4. **Copy.** Headline punctuation is removed. Miner percentages explicitly mean
   share of miners, rather than a positive change. French miner labels are
   harmonised. No performance claims or invented measurements were added.
5. **Type.** Locally served IBM Plex Sans (400/500/600) and IBM Plex Mono
   (400/500) replace heavy, tracked type. Figures use tabular numerals.
6. **Layout.** The first screen places copy beside the product. Pool groups the
   node, service status and timestamp together. Miner presents the existing
   Bitaxe photo with a separate, clearly labelled community reading. Mobile
   follows the reading order. The original sections remain in their order.
7. **Details.** Control and panel radii, tables, selections, focus rings and
   captions share one system. The hashrate chart uses a quiet grid and orange
   series; every actual measurement and missing-data gap is retained.
8. **Visuals.** The photograph is the project's existing
   `installer/assets/boards/gamma.webp`. The on-device previews remain. Pool's
   visual is its actual node verification panel. No image was generated and no
   image prompt was used.
9. **Motion.** Hover feedback uses named color/border transitions. Data panels
   no longer tilt, shortcut links no longer rise, and the progress width no
   longer animates. Existing meaningful loading and accessibility behavior is
   preserved, including reduced motion and keyboard chart inspection.

## Sources and verification

The visual direction is unchanged by the Rust migration. Edit shared and
translated HTML in `site/templates/` and browser assets in `assets/`. The
Rust generator (`site/Cargo.toml`) builds all pages and public resources into
`out/` without React or Next.js. Cargo checks and the existing browser suite
validate routes, forms, themes and BTC/BCH behavior before publication.
See `site/README.md` for build, test and deployment commands.

Design method: **unslop by Damir Sokolovsky ([@heydamir](https://x.com/heydamir))**,
licensed CC BY 4.0. https://heydamir.design/unslop
