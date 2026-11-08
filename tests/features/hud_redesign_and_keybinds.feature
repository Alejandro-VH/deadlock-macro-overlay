Feature: HUD Streamlining, Configurable Keybinds, and Simultaneous Milestone Display
  As a competitive Deadlock player
  I want an uncluttered vertical HUD, configurable farm hotkeys, and grouped simultaneous timers
  So that I can track rotational objectives at a glance without visual noise or missed milestones

  Background:
    Given the overlay is running and attached to an active Deadlock session

  Scenario: Grouping simultaneous milestones under a single target time
    Given the match clock reaches 9 minutes and 39 seconds
    When the overlay computes the immediate objectives for the 10-minute mark
    Then all milestones sharing that target time are grouped under a single countdown
    And the alert is presented without exclamation marks or uppercase text prefixes

  Scenario: Previewing all milestones at the subsequent milestone
    Given multiple objectives will unlock simultaneously at the 15-minute mark
    When the overlay projects the next objective milestone beyond the current one
    Then all simultaneous objectives for that next milestone are included in the preview

  Scenario: Fixed presence of farm timers in idle state
    Given the player has not triggered any farm route or boss timers
    When the overlay renders the status section
    Then the box route tracker is displayed with an idle state
    And the Mid Boss tracker is displayed with an idle state

  Scenario: Window-scoped passive hotkey trigger for box farm
    Given Deadlock is the active foreground window
    When the player presses the configured box route key
    Then the box route 3-minute countdown begins
    And the key press is not blocked from reaching the game client

  Scenario: Ignoring hotkey triggers when Deadlock is not in foreground
    Given another application is active in the foreground
    When the player presses the configured box route key
    Then the box route timer state remains unchanged

  Scenario: Interactive keybinding via configuration menu
    Given the overlay configuration panel is open
    When the player assigns a new key to the box route timer
    Then the key configuration is updated and saved for subsequent sessions
