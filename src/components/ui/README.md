# Keyturn UI components

Adapted from the free [Shadcn Studio components](https://shadcnstudio.com/docs/components),
listed in [awesome-shadcn/ui](https://awesomeshadcn.dev/).
Upstream source: [themeselection/shadcn-studio](https://github.com/themeselection/shadcn-studio/tree/main/src/components/ui).
The upstream license is preserved in [LICENSE.md](./LICENSE.md).

Tabs, Slider, Switch, ToggleGroup, Dialog, Button, Avatar, Skeleton and Progress
keep the shadcn composition model and Radix interaction primitives. Their styles
use Keyturn's existing Figtree, appearance and accent tokens in `src/index.css`.
Only the APIs used by Keyturn are included. Dialogs restore focus to the button
that opened them, even though they open from app state rather than a DialogTrigger.

Refresh cadence uses the same segmented control as Theme, with 30, 60 and 120
second presets. A previously saved custom cadence is included as a selected
option. Radix provides keyboard navigation and radio semantics; changes save
through `onValueChange`, with all options disabled while saving.

Settings uses an iOS-style sliding indicator with the existing Radix primitives,
following the composition of [Animate UI's Radix Tabs](https://animate-ui.com/docs/components/radix/tabs),
also listed on awesome-shadcn. The indicator uses a CSS transform with the iOS
easing curve, so rapid selection changes smoothly interrupt the current movement.
Keyboard tab navigation skips motion; reduced-motion settings disable movement.
Compact appearance and update grouping were informed by Mobbin's
[Opera appearance screen](https://mobbin.com/screens/676696fc-385a-4baa-82ea-0ba158001ec6)
and [Philips Hue update screen](https://mobbin.com/screens/d55ae74c-1800-4647-98c9-69c0b9bb539a).
