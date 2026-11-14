# minnow-settings

Application preferences and persistence.

- Defines the settings schema and defaults.
- Serializes writes and preserves the previous file when persistence fails.
- Exposes persistence status and bounded shutdown.

Stores desired preferences. Platform adapters report actual OS state.
