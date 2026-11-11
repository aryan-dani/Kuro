use kuro_router::{has_word, normalize};

#[test]
fn section_5_1_normalization_matrix() {
    let cases = [
        ("Daddy's home", "daddy home"),
        ("daddys back", "daddy back"),
        ("dadi is here", "daddy is here"),
        ("wake up daddy", "wakeup daddy"),
        ("wake-up please", "wakeup please"),
        ("woke up now", "wakeup now"),
        ("good night sir", "goodnight sir"),
        ("in his home", "in daddy home"),
        ("what's the news", "whats the news"),
        ("moniter please", "monitor please"),
        ("extention off", "extension off"),
        ("monitor of please", "monitor off please"),
        ("let's off", "desk off"),
        ("lets off", "desk off"),
        ("bob it off", "power off"),
        ("urgent man", "urgent mail"),
        ("Send my Clipboard!", "send my clipboard"),
    ];
    for (input, want) in cases {
        assert_eq!(normalize(input), want, "normalize({input})");
    }
    assert!(has_word("clipboard on", "on"));
    assert!(!has_word("clipboard", "on"));
}
