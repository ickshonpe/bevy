---
title: "Font weight support"
authors: ["@ickshonpe"]
pull_requests: [22038]
---

`bevy_text` now supports font weights. You can set the weight using the new `weight: FontWeight` field on `TextFont`. `FontWeight` newtypes a `u16`, valid values range from 1 to 1000, inclusive. Values outside the range are clamped.
