//! Opt-in IME receiving-window experiment. No text persistence or automatic paste.
//! A release is an audio fact, never proof that the IME finished recognition.

#[derive(Debug, Default)]
pub(crate) struct ProbeSession {
    pub generation: u64,
    pub released: bool,
    pub composing: bool,
    pub chars: usize,
}

impl ProbeSession {
    pub fn begin(&mut self, generation: u64) {
        *self = Self {
            generation,
            ..Self::default()
        };
    }
    pub fn release(&mut self, generation: u64) -> bool {
        if generation == 0 || generation != self.generation || self.released {
            return false;
        }
        self.released = true;
        true
    }
    pub fn input(&mut self, generation: u64, chars: usize, composing: bool) -> bool {
        if generation == 0 || generation != self.generation {
            return false;
        }
        self.chars = chars;
        self.composing = composing;
        true
    }
    pub fn cancel(&mut self) {
        *self = Self::default();
    }
}

#[cfg(windows)]
#[path = "clipboard_probe_windows.rs"]
mod native;
#[cfg(windows)]
pub use native::{begin, cancel, enabled, release, set_enabled, shutdown};
#[cfg(not(windows))]
pub fn enabled() -> bool {
    false
}
#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool, _english: bool) -> Result<bool, String> {
    Err("windows_required".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_press_clears_old_release_and_text_state() {
        let mut state = ProbeSession::default();
        state.begin(1);
        assert!(state.input(1, 20, true));
        assert!(state.release(1));
        assert!(!state.release(1));
        state.begin(2);
        assert_eq!(state.generation, 2);
        assert!(!state.released);
        assert!(!state.composing);
        assert_eq!(state.chars, 0);
    }

    #[test]
    fn late_release_and_input_cannot_change_a_newer_press() {
        let mut state = ProbeSession::default();
        state.begin(2);
        assert!(!state.release(1));
        assert!(!state.input(1, 99, true));
        assert!(!state.released);
        assert_eq!(state.chars, 0);
    }

    #[test]
    fn release_keeps_accepting_delayed_ime_results() {
        let mut state = ProbeSession::default();
        state.begin(1);
        assert!(state.release(1));
        assert!(state.input(1, 24, false));
        assert!(state.released);
        assert_eq!(state.chars, 24);
    }

    #[test]
    fn cancel_invalidates_late_messages_and_is_idempotent() {
        let mut state = ProbeSession::default();
        state.begin(1);
        state.cancel();
        state.cancel();
        assert_eq!(state.generation, 0);
        assert!(!state.input(1, 24, false));
        assert!(!state.release(1));
    }
}
