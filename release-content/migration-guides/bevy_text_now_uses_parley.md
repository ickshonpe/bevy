---
title: "`bevy_text` migration from Cosmic Text to Parley"
pull_requests: [22879]
---

`bevy_text` now uses Parley for its text layout. For the most part, this change should be invisible to users of `bevy_text` and Bevy more broadly.

However, some low-level public methods and types (such as `FontAtlasKey`) have changed to map to `parley`'s distinct API.

This migration should be relatively straightforward. Use the linked PR as an example of the correct migration, but please ask for help (and explain your use case) if you run into difficulties.
