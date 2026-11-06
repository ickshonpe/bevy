---
title: "Val refactor"
pull_requests: [26409]
---

- `LayoutContext` has been renamed to `ValContext`.
- `Val::resolve`, `Val2::resolve`, `UiPosition::resolve`, `CornerRadius::resolve`,
  `RadialGradientShape::resolve`, `BorderRadius::resolve`, and `UiTransform::compute_affine`
  now take `&ValContext` instead of separate scale factor, target size, `EmSize`, and `RemSize`
  arguments. The basis for percent values remains a separate argument.
- `Val` has been moved from the `bevy_ui` crate's `geometry` module to a new module `val`.
- `TaffyStyle` no longer implements `Clone`.
- `Val` is no longer an `enum`. It is packed into eight bytes so that it can hold expressions.
  Construct values with `Val::AUTO`, `Val::px`, `Val::percent`, `Val::vw`, `Val::vh`, `Val::vmin`,
  `Val::vmax`, `Val::em` and `Val::rem`, and match on them through `Val::unpack`, which returns a
  `ValRef` enum with the former variants:

  ```rust
  // 0.18
  if let Val::Px(px) = node.width {
      node.width = Val::Px(px + 10.0);
  }

  // 0.19
  if let ValRef::Px(px) = node.width.unpack() {
      node.width = Val::px(px + 10.0);
  }
  ```
