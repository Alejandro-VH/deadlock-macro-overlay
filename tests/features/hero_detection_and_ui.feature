Feature: Deadlock Hero Detection and Noir UI
  As a competitive Deadlock player
  I want the overlay to automatically detect my chosen hero and display clean macro status
  So that I have tailored situational awareness without visual distractions or manual setup

  Background:
    Given the overlay is active and monitoring the game

  Scenario: Automatic hero detection from game spawn event
    Given a player selects a hero before entering the match
    When the game registers the hero spawn event for "hero_hornet"
    Then the overlay identifies the active hero as "Vindicta"
    And the hero selection mode is marked as automatic

  Scenario: Automatic hero detection for brawler heroes
    Given a player joins a match
    When the game registers the hero spawn event for "hero_inferno"
    Then the overlay identifies the active hero as "Infernus"
    And the hero selection mode is marked as automatic

  Scenario: Unknown engine hero fallback
    Given a match starts with an unmapped experimental hero
    When the game registers the hero spawn event for "hero_unknown_character"
    Then the overlay displays the cleaned hero name "Unknown Character"
    And the hero selection mode is marked as automatic

  Scenario: Manual hero override by player
    Given the overlay detected "Vindicta" automatically
    When the player manually chooses "Seven" from the hero menu
    Then the overlay updates the active hero to "Seven"
    And the hero selection mode is marked as manual

  Scenario: Preserving manual hero selection across log updates
    Given the player manually selected "Abrams"
    When the game writes a background spawn event for another character
    Then the overlay retains "Abrams" as the active hero

  Scenario: Compact HUD presentation
    Given the match clock is at 5 minutes and 32 seconds
    When the player views the macro overlay
    Then the display shows the synchronized clock and the immediate next milestone
    And the progress bar visualizes completion without bright border artifacts
