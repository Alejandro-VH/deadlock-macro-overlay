use deadlock_overlay::tray::{
    acquire_single_instance_lock, map_menu_command, TrayAction, ID_TRAY_EXIT, ID_TRAY_RELOAD,
    ID_TRAY_RESET, TRAY_MENU_EXIT, TRAY_MENU_RELOAD, TRAY_MENU_RESET,
};

#[test]
fn test_tray_menu_labels_are_clean_without_parentheses() {
    for label in [TRAY_MENU_RESET, TRAY_MENU_RELOAD, TRAY_MENU_EXIT] {
        assert!(!label.contains('(') && !label.contains(')'));
    }
    assert_eq!(TRAY_MENU_RESET, "Reiniciar partida");
    assert_eq!(TRAY_MENU_RELOAD, "Recargar configuración");
    assert_eq!(TRAY_MENU_EXIT, "Salir");
}

#[test]
fn test_map_menu_command_resolves_all_tray_actions() {
    assert_eq!(map_menu_command(ID_TRAY_RESET), Some(TrayAction::ResetMatch));
    assert_eq!(map_menu_command(ID_TRAY_RELOAD), Some(TrayAction::ReloadConfig));
    assert_eq!(map_menu_command(ID_TRAY_EXIT), Some(TrayAction::ExitApp));
    assert_eq!(map_menu_command(9999), None);
}

#[test]
fn test_single_instance_lock_blocks_second_acquisition_until_dropped() {
    let mutex_name = "Local\\DeadlockMacroOverlay_TestMutex_Unique";
    let first_guard = acquire_single_instance_lock(mutex_name);
    assert!(first_guard.is_some(), "First instance must acquire mutex");

    let second_guard = acquire_single_instance_lock(mutex_name);
    assert!(second_guard.is_none(), "Second instance must return None passively");

    drop(first_guard);
    let third_guard = acquire_single_instance_lock(mutex_name);
    assert!(third_guard.is_some(), "Mutex must be re-acquirable after drop");
}
