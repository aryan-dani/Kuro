use kuro_hotkey::{PttMachine, parse_hotkey};

#[test]
fn ctrl_alt_space_and_escape_cancel() {
    let spec = parse_hotkey("ctrl+alt+space");
    assert!(spec.ctrl && spec.alt && spec.vk == 0x20);
    let mut machine = PttMachine::new();
    assert!(machine.on_hotkey_down());
    let epoch = machine.clock.snapshot();
    machine.on_release();
    machine.on_escape();
    assert!(!machine.clock.live(epoch));
}
