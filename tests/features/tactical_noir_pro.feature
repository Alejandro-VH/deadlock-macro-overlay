Feature: Tactical Noir Pro HUD, Input Isolation, and Precise Match Synchronization
  As a competitive Deadlock player
  I want an elegant, high-contrast, distraction-free macro HUD with precise clock synchronization and robust input isolation
  So that I have actionable objective timings without keyboard interaction conflicts or visual clutter

  Scenario: Match clock updates only on second boundary changes
    Given the match clock is synchronized at 4 minutes and 12 seconds
    When an OCR reading arrives with the exact same timestamp 4 minutes and 12 seconds
    Then the internal clock tick interpolation is not restarted prematurely

  Scenario: Standby mode presents clean waiting state and close control
    Given Deadlock is not currently running on the system
    When the player views the macro overlay
    Then the top progress bar is hidden and a close control is available

  Scenario: Active milestone displays badge capsule instead of textual suffix
    Given a macro milestone reaches its scheduled target time
    When the milestone enters its active window
    Then an active status badge is displayed alongside the milestone time

  Scenario: Milestone rotation warning displays pronto badge
    Given a macro milestone has 25 seconds remaining before activation
    When the milestone status is evaluated
    Then a warning status badge is displayed alongside the milestone time

  Scenario: Silencing 30-second warning alert preserves active chime
    Given the player has disabled 30-second warning sounds in overlay settings
    When a milestone crosses the 30-second warning threshold
    Then no audio warning chime is triggered but the visual status updates

  Scenario: Overlay configuration persists scale and opacity settings
    Given the player adjusts the HUD scale to 115 percent and background opacity to 75 percent
    When the configuration is saved to disk
    Then subsequent application launches restore the configured scale and opacity
