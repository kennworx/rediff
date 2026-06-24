## ADDED Requirements

### Requirement: Built-in themes
The system SHALL provide at least one dark and one light built-in theme and SHALL let the user
switch the active theme at runtime.

#### Scenario: Switch theme at runtime
- **WHEN** the user opens the theme selector and chooses a different built-in theme
- **THEN** the UI re-renders with the selected theme's colors

### Requirement: Configuration file
The system SHALL read persisted preferences from `~/.config/rsdiff/config.toml`, including at
least theme, layout mode, line numbers, and line wrapping.

#### Scenario: Config applied at startup
- **WHEN** a config file sets a theme and layout mode
- **THEN** the system starts with those preferences applied

#### Scenario: Missing config uses defaults
- **WHEN** no config file is present
- **THEN** the system starts with built-in defaults and does not error

### Requirement: CLI flags override config
Command-line flags SHALL override the corresponding config-file values for a given invocation.

#### Scenario: Flag beats config
- **WHEN** the config sets one layout mode and the user passes a different mode flag
- **THEN** the flag's mode is used for that invocation
