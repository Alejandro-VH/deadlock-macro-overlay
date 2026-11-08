Feature: Farm Route Timing, Hero Detection, and In-Game Usability
  As a competitive Deadlock player
  I want single-key farm timers, accurate local hero detection, and frictionless Tab interaction
  So that I can optimize rotations and interact with the overlay without disrupting combat

  Background:
    Given the overlay is active and monitoring Deadlock

  Scenario: Toggling box route countdown with a single key
    Given the match clock is at 4 minutes
    When the player triggers the box route timer
    Then a 3-minute box respawn timer begins counting down

  Scenario: Cancelling active box route countdown with the same key
    Given an active box route timer is counting down
    When the player triggers the box route timer again
    Then the box route timer is cancelled immediately

  Scenario: Toggling Mid Boss tracker with a single key
    Given the match clock is at 11 minutes
    When the player triggers the Mid Boss elimination key
    Then the Mid Boss respawn timer is registered for 7 minutes

  Scenario: Cancelling active Mid Boss tracker with the same key
    Given an active Mid Boss respawn timer is counting down
    When the player triggers the Mid Boss elimination key again
    Then the Mid Boss timer is cancelled immediately

  Scenario: Accurate local player hero detection ignoring server bot logs
    Given a match loads with multiple bots and players
    When the local client requests spawning "hero_tengu"
    Then the overlay identifies the active player hero as "Ivy"
    And ignores any subsequent bot spawn events from the server

  Scenario: Automatic match reset upon game disconnect
    Given an active match is being tracked with non-zero game time
    When the game client disconnects from the match server
    Then the match clock and all active respawn timers are reset to idle

  Scenario: Dual horizon macro milestone display
    Given the match clock reaches 4 minutes and 45 seconds
    When the player looks at the macro overlay
    Then the imminent "Bridge Buffs" rotation is displayed
    And the subsequent "Jungla T2" milestone is displayed as the next objective
