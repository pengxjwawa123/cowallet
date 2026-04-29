# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

cowallet is a **single-file static HTML prototype** for an AI-native crypto wallet app concept. The entire application lives in `index.html` (~3100 lines) — no build system, no package manager, no framework.

Tagline: "the wallet that reads you back"

## Architecture

The file is structured in three sections:

1. **CSS** (lines ~10–1980): Full design system using CSS custom properties (`--bg-paper`, `--accent`, `--ink-*`, etc.) with a serif-led typography stack (Noto Serif SC, Fraunces, Inter, JetBrains Mono). Styled as a phone-frame mockup with a paper/ink palette inspired by Claude's design language.

2. **HTML** (lines ~1980–1982): Semantic structure with two top-level containers:
   - `#onb` — Multi-stage onboarding flow (hero → start → create/import → biometrics → name → persona)
   - `#app` — Main app with views: `home`, `wallet`, `agents`, `settings`, `keys`, `chat`

3. **JavaScript** (lines ~1983–3097): Vanilla JS with no dependencies. Key systems:
   - **State**: Single mutable `state` object (`lang`, `userName`, `persona`, `tab`, `view`, `intentMode`, `attachedImg`)
   - **Navigation**: `setView(view)` switches between views; `VIEW_META` maps views to header configs and parent tabs
   - **Onboarding**: Event-delegated via `data-onb` attributes; `showStage(stage)` transitions between stages
   - **Chat/Intent**: `INTENT_RULES` array with regex patterns (zh/en) that detect user intent (savings, transfer, spending, balance, etc.) and render intent confirmation cards
   - **Composer**: Text input with image upload, voice input simulation, and intent-mode toggle (enter vs. live)
   - **Demo**: Automated walkthrough (`demo.run()`) that simulates cursor movements and taps through all features

## Bilingual System

All UI text uses a CSS-driven bilingual pattern:
- Elements have `data-zh="中文"` and `data-en="English"` attributes
- CSS `::before` pseudo-elements render the active language based on `body.lang-zh` / `body.lang-en` class
- Placeholders use `data-zh-placeholder` / `data-en-placeholder` attributes
- JS intent rules have separate `re_zh` / `re_en` regex patterns

## Key Conventions

- DOM selection: `$` = `querySelector`, `$$` = `querySelectorAll` (defined at top of script)
- Navigation: `data-nav="viewname"` attributes trigger `setView()` via global click delegation
- Onboarding: `data-onb="action"` attributes trigger stage transitions via event delegation on `#onb`
- Tab bar items use `data-tab="tabname"` attributes
- All colors use CSS custom properties from `:root`
- No external JS dependencies — everything is self-contained

## Development

Open `index.html` directly in a browser. No server required (though a local server avoids CORS issues with clipboard API).

```
# Simple local server options:
python3 -m http.server 8000
npx serve .
```
