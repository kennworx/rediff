## ADDED Requirements

### Requirement: Single active input mode
The system SHALL track a single active input context that determines how keyboard and mouse input is routed. That context is a base (the normal stream — focused on the diff or the sidebar — or the file peek) with at most one transient overlay (the fuzzy palette or help) layered on top. When an overlay is present it is the active context; otherwise the base is. Keyboard and mouse routing SHALL derive from this same context, with one precedence, so the two input paths never disagree about which context is active.

#### Scenario: Keyboard routes to the active overlay
- **WHEN** an overlay (the palette or help) is open and the user presses a key
- **THEN** the key is interpreted by that overlay's bindings, not the base's

#### Scenario: Mouse does not leak through an overlay
- **WHEN** the fuzzy palette or help overlay is open and the user scrolls the wheel or clicks
- **THEN** the event is handled by (or absorbed for) the active overlay and does not scroll or select within the diff behind it

### Requirement: Overlays layer over a retained base
An overlay (the palette or help) SHALL be opened over a base context that is retained while the overlay is active, so the overlay's mode-dependent content reflects that base and closing the overlay returns to it. The help overlay in particular SHALL present the bindings of the base it was opened over.

#### Scenario: Help reflects the base beneath it
- **WHEN** the user opens help while the file peek is the active base
- **THEN** the help lists the peek's bindings; **and WHEN** the user opens help while the normal stream is the active base, the help lists the stream's bindings

#### Scenario: Closing an overlay returns to its base
- **WHEN** the user opens an overlay over a base and then dismisses the overlay
- **THEN** the active context is the same base it was opened over, in the same state, not a default or reset context

### Requirement: Status line reflects the active mode
The status line SHALL show hints and context for the active mode. While the file peek is open it SHALL show the peek's context (the peeked file and the peek's own position) and the peek's bindings, not the underlying stream's; the same SHALL hold for other overlay modes.

#### Scenario: Peek shows peek status
- **WHEN** the file peek is open
- **THEN** the status line describes the peek (its file and scroll position) and shows keys that act in the peek, not the stream's file count, scroll position, or stream keys

#### Scenario: Returning to the stream restores stream status
- **WHEN** the user closes the peek
- **THEN** the status line again shows the stream's file count, position, and stream bindings

### Requirement: Status percentage tracks the active layout
The scroll-position percentage shown in the status line SHALL be computed against the currently displayed layout's row count.

#### Scenario: Percentage correct in split layout
- **WHEN** the user is in the side-by-side (split) layout and scrolls
- **THEN** the status percentage reflects position within the split layout's rows, not the stacked layout's

### Requirement: One keymap definition drives behavior, hints, and help
The keybindings SHALL be defined in one place, and the status-line hints and the help overlay SHALL be derived from that definition rather than maintained as independent copies, so they cannot drift from the bindings that are actually in effect.

#### Scenario: Help matches actual bindings
- **WHEN** the user opens the help overlay
- **THEN** the keys it lists are the keys the active routing actually handles

### Requirement: Exactly one overlay is shown
At most one overlay (peek, palette, or help) SHALL be displayed at a time, selected by the active mode, so overlays cannot visually stack and input precedence cannot disagree with what is drawn.

#### Scenario: Opening an overlay replaces, never stacks
- **WHEN** the active mode is an overlay mode
- **THEN** only that overlay is rendered over the body, and it is the overlay receiving input
