use std::collections::HashMap;
use std::sync::OnceLock;

use super::Intent;

pub(crate) fn tier1_map() -> &'static HashMap<&'static str, Intent> {
    static MAP: OnceLock<HashMap<&'static str, Intent>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        for (k, v) in [
            ("cancel", Intent::Cancel),
            ("nevermind", Intent::Cancel),
            ("never mind", Intent::Cancel),
            ("forget it", Intent::Cancel),
            ("stop", Intent::Cancel),
            ("abort", Intent::Cancel),
            ("thanks", Intent::Thanks),
            ("thank you", Intent::Thanks),
            ("cheers", Intent::Thanks),
            ("help", Intent::Help),
            ("commands", Intent::Help),
            ("who are you", Intent::Identity),
            ("what are you", Intent::Identity),
            ("time", Intent::Time),
            ("the time", Intent::Time),
            ("date", Intent::Date),
            ("the date", Intent::Date),
            ("pause", Intent::PauseMusic),
            ("pause song", Intent::PauseMusic),
            ("pause music", Intent::PauseMusic),
            ("resume", Intent::ResumeMusic),
            ("resume song", Intent::ResumeMusic),
            ("stop music", Intent::StopMusic),
            ("stop song", Intent::StopMusic),
            ("good night", Intent::GoodNight),
            ("goodnight", Intent::GoodNight),
            ("desk on", Intent::PowerOn),
            ("desk off", Intent::PowerOff),
            ("lights on", Intent::PowerOn),
            ("lights off", Intent::PowerOff),
            ("power on", Intent::PowerOn),
            ("power off", Intent::PowerOff),
        ] {
            m.insert(k, v);
        }
        m
    })
}
