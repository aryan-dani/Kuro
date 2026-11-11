use kuro_wifi::WifiPolicy;

#[test]
fn disabled_and_away_flags() {
    let dir = std::env::temp_dir().join(format!("kuro-wifi-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("home_ssid.txt"), "HomeNet\n").unwrap();
    let policy = WifiPolicy::load(&dir);
    assert!(!policy.should_run(Some("Cafe")));
    assert!(policy.should_run(Some("homenet")));
    std::fs::write(dir.join("AWAY_OK"), "").unwrap();
    let policy = WifiPolicy::load(&dir);
    assert!(policy.should_run(Some("Cafe")));
    std::fs::write(dir.join("DISABLED"), "").unwrap();
    let policy = WifiPolicy::load(&dir);
    assert!(!policy.should_run(Some("HomeNet")));
    let _ = std::fs::remove_dir_all(&dir);
}
