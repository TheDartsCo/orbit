# Orbit Logo Theme Design

## Goal

Align Orbit's application chrome with the new constellation logo without using
gradients or weakening the existing agent color system.

## Direction

Orbit keeps its dense, dark developer-tool layout. Neutral gray surfaces shift
to near-black navy and violet tones drawn from the app icon. Orbit-owned
interaction states use the logo's electric blue and indigo-violet colors.

Agent badges and agent-specific labels remain unchanged so users can identify
agents quickly.

## Palette

- Primary background: `#0B0B12`
- Secondary background: `#11111A`
- Tertiary background: `#191927`
- Elevated background: `#222233`
- Hover background: `#292940`
- Active background: `#333354`
- Border: `#2B2B42`
- Light border: `#414163`
- Primary text: `#F3F4F8`
- Secondary text: `#B8B9CB`
- Muted text: `#7F8098`
- Accent: `#38BDF8`
- Accent hover: `#67D2F8`

Search highlights use a muted magenta tint. Coral remains reserved for
destructive and error states.

## Scope

- Update shared Tailwind theme tokens.
- Replace hard-coded neutral main, transcript, code, and tool-call backgrounds.
- Replace yellow search-result highlights with magenta.
- Keep agent color maps unchanged.
- Add no gradients or glow.

## Verification

- Audit frontend files for replaced hard-coded neutral surfaces and yellow
  search highlights.
- Run `npm run build`.
- Inspect the local frontend visually.
