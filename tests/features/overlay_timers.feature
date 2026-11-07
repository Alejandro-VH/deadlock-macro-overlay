Feature: Deadlock Macro Event Tracking and Timing
  As a competitive Deadlock player
  I want an automated overlay tracking match time and global objectives
  So that I can anticipate rotations without manual stopwatches

  Background:
    Given a fresh match has started

  Scenario: Initial Bridge Buffs spawn alert at five minutes
    Given the match clock reaches 4 minutes and 30 seconds
    When the macro scheduler evaluates upcoming events
    Then a warning rotation alert for "Bridge Buffs" is scheduled with 30 seconds remaining

  Scenario: Bridge Buffs activate at five minutes
    Given the match clock reaches 5 minutes exactly
    When the macro scheduler evaluates upcoming events
    Then the active alert displays "Bridge Buffs" as active now

  Scenario: Bridge Buffs cycle every five minutes thereafter
    Given the match clock reaches 7 minutes exactly
    When the macro scheduler evaluates upcoming events
    Then the next "Bridge Buffs" spawn is scheduled for 10 minutes exactly

  Scenario: Soul Urn initial spawn at ten minutes
    Given the match clock reaches 9 minutes and 30 seconds
    When the macro scheduler evaluates upcoming events
    Then a warning rotation alert for "Soul Urn" is scheduled with 30 seconds remaining

  Scenario: Simultaneous events at ten minutes combined display
    Given the match clock reaches 10 minutes exactly
    When the macro scheduler evaluates upcoming events
    Then both "Bridge Buffs" and "Soul Urn" are displayed as active simultaneously

  Scenario: Sinner's Sacrifice initial appearance at eight minutes
    Given the match clock reaches 8 minutes exactly
    When the macro scheduler evaluates upcoming events
    Then the active alert displays "Sinner's Sacrifice" as active now

  Scenario: Sinner's Sacrifice expires from the queue after ten seconds
    Given the match clock reaches 8 minutes and 11 seconds
    When the macro scheduler evaluates upcoming events
    Then "Sinner's Sacrifice" is no longer present in the active or upcoming queue

  Scenario: First Mid Boss elimination starts seven minute countdown
    Given the match clock is at 12 minutes
    When the player registers the first Mid Boss elimination
    Then the Mid Boss respawn timer is set for 7 minutes

  Scenario: Second Mid Boss elimination starts six minute countdown
    Given the first Mid Boss was eliminated earlier
    And the match clock is at 20 minutes
    When the player registers the second Mid Boss elimination
    Then the Mid Boss respawn timer is set for 6 minutes

  Scenario: Third Mid Boss elimination starts five minute countdown
    Given two Mid Bosses have been eliminated earlier
    And the match clock is at 28 minutes
    When the player registers the third Mid Boss elimination
    Then the Mid Boss respawn timer is set for 5 minutes

  Scenario: Erroneous Mid Boss elimination is cancelled
    Given a Mid Boss elimination countdown is currently active
    When the player cancels the Mid Boss timer
    Then the Mid Boss countdown is deactivated immediately

  Scenario: Clock drift compensation when screen reading is occluded
    Given the match clock was reading 6 minutes and 15 seconds
    And screen reading becomes occluded for 3 seconds
    When the internal clock continues interpolating
    Then the current match time advances to 6 minutes and 18 seconds

  Scenario: Match reset clears all state
    Given a match has progressed for 15 minutes with one Mid Boss kill
    When the player resets the match
    Then the match clock returns to 0 minutes and 0 seconds
    And all active alerts and Mid Boss kill counters are cleared
