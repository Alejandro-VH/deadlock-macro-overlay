Feature: Application Identity, Single-Instance Protection, and System Tray Lifecycle
  As a Deadlock player
  I want a self-contained desktop application with proper system identity, automatic user-scoped configuration, and system tray controls
  So that I can manage the overlay effortlessly without duplicate instances, false security alarms, or cluttered text

  Scenario: Initial launch creates default configuration for the display resolution
    Given the player launches the overlay for the first time on a system without existing configuration
    When the application initializes its settings for the primary display resolution
    Then a new configuration is persisted automatically in the user profile with resolution-calibrated coordinates

  Scenario: Subsequent launch while already running exits passively
    Given an instance of the overlay is already running in the background
    When the player attempts to launch a second instance of the application
    Then the second instance terminates immediately without opening a duplicate window

  Scenario: Player resets the active match from the system tray
    Given an active match is in progress with running objective and farm timers
    When the player selects "Reiniciar partida" from the system tray menu
    Then the match clock and all objective trackers return to their initial standby state

  Scenario: Player reloads configuration from the system tray
    Given the player has modified the persisted configuration settings
    When the player selects "Recargar configuración" from the system tray menu
    Then the overlay applies the updated configuration values immediately

  Scenario: Player closes the application from the system tray
    Given the overlay is running in the system tray
    When the player selects "Salir" from the system tray menu
    Then the system tray icon is removed and the application shuts down cleanly

  Scenario: Key rebinding prompt uses clean text without parenthetical explanations
    Given the player opens the configuration panel to customize hotkeys
    When the player views the key rebinding cancellation hint
    Then the hint displays "Esc para cancelar" without parenthetical text
