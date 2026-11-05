//! IME receiver lifecycle. Delivery requires both button release and IME commit.
//! A release alone is never proof that the IME finished recognition.

#[derive(Debug, Default)]
pub(crate) struct ProbeSession {
    pub generation: u64,
    pub released: bool,
    pub composing: bool,
    pub chars: usize,
    pub ime_finished: bool,
    pub revision: u64,
    pub delivered: bool,
    pub auto_paste: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeliveryFailure {
    CopyFailed,
    SourceChanged,
    TargetChanged,
    RestoreFailed,
    ClipboardChanged,
    PasteRejected,
    Interrupted,
    ModifiersHeld,
    HideFailed,
}
impl DeliveryFailure {
    pub fn token(self) -> &'static str {
        match self {
            Self::CopyFailed => "copy_failed",
            Self::SourceChanged => "source_changed",
            Self::TargetChanged => "target_changed",
            Self::RestoreFailed => "restore_failed",
            Self::ClipboardChanged => "clipboard_changed",
            Self::PasteRejected => "paste_rejected",
            Self::Interrupted => "interrupted",
            Self::ModifiersHeld => "modifiers_held",
            Self::HideFailed => "hide_failed",
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DeliveryOutcome {
    ClipboardBusy,
    Submitted,
    Kept(DeliveryFailure),
}
pub(crate) trait DeliveryHost {
    /// None means the clipboard is temporarily locked. Errors are terminal.
    fn copy_verified(&mut self, text: &str) -> Result<Option<()>, DeliveryFailure>;
    fn allow_restore(&mut self) -> Result<(), DeliveryFailure>;
    fn hide_receiver(&mut self) -> bool;
    fn restore_target(&mut self) -> bool;
    fn verify_before_paste(&mut self) -> Result<(), DeliveryFailure>;
    fn paste_once(&mut self) -> bool;
    fn retain_receiver(&mut self);
}
pub(crate) fn deliver_with(host: &mut impl DeliveryHost, text: &str) -> DeliveryOutcome {
    match host.copy_verified(text) {
        Ok(None) => return DeliveryOutcome::ClipboardBusy,
        Err(reason) => return DeliveryOutcome::Kept(reason),
        Ok(Some(())) => {}
    }
    if let Err(reason) = host.allow_restore() {
        return DeliveryOutcome::Kept(reason);
    }
    if !host.hide_receiver() {
        return DeliveryOutcome::Kept(DeliveryFailure::HideFailed);
    }
    let result = if !host.restore_target() {
        Err(DeliveryFailure::RestoreFailed)
    } else {
        host.verify_before_paste().and_then(|_| {
            if host.paste_once() {
                Ok(())
            } else {
                Err(DeliveryFailure::PasteRejected)
            }
        })
    };
    match result {
        Ok(()) => DeliveryOutcome::Submitted,
        Err(reason) => {
            host.retain_receiver();
            DeliveryOutcome::Kept(reason)
        }
    }
}

impl ProbeSession {
    pub fn begin(&mut self, generation: u64) {
        *self = Self {
            generation,
            auto_paste: true,
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
        if generation == 0 || generation != self.generation || self.delivered {
            return false;
        }
        self.chars = chars;
        self.composing = composing;
        self.revision = self.revision.wrapping_add(1);
        if composing {
            self.ime_finished = false;
        }
        true
    }
    pub fn ime_end(&mut self, generation: u64) -> bool {
        if generation == 0 || generation != self.generation || self.delivered {
            return false;
        }
        self.composing = false;
        self.ime_finished = true;
        true
    }
    pub fn claim_delivery(&mut self, generation: u64, revision: u64) -> bool {
        if generation == 0
            || generation != self.generation
            || revision != self.revision
            || !self.released
            || !self.ime_finished
            || self.composing
            || self.chars == 0
            || self.delivered
        {
            return false;
        }
        self.delivered = true;
        true
    }
    pub fn stop_auto_paste(&mut self) {
        self.auto_paste = false;
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

    #[test]
    fn release_waits_for_ime_commit_without_any_fixed_delay() {
        let mut state = ProbeSession::default();
        state.begin(1);
        state.input(1, 12, true);
        state.release(1);
        assert!(!state.claim_delivery(1, state.revision));
        assert!(state.ime_end(1));
        state.input(1, 18, false);
        assert!(state.claim_delivery(1, state.revision));
        assert!(!state.claim_delivery(1, state.revision));
    }

    #[test]
    fn a_committed_result_can_precede_the_ble_release_notification() {
        let mut state = ProbeSession::default();
        state.begin(1);
        state.input(1, 9, false);
        state.ime_end(1);
        assert!(!state.claim_delivery(1, state.revision));
        state.release(1);
        assert!(state.claim_delivery(1, state.revision));
    }

    #[test]
    fn empty_and_stale_results_never_paste_and_new_composition_invalidates_a_commit() {
        let mut state = ProbeSession::default();
        state.begin(1);
        state.release(1);
        state.ime_end(1);
        assert!(!state.claim_delivery(1, state.revision));
        state.input(1, 9, false);
        let old_revision = state.revision;
        state.input(1, 12, true);
        assert!(!state.claim_delivery(1, old_revision));
        assert!(!state.claim_delivery(1, state.revision));
        state.ime_end(1);
        assert!(state.claim_delivery(1, state.revision));
        state.begin(2);
        assert!(!state.claim_delivery(1, state.revision));
        assert!(!state.ime_end(1));
    }

    #[test]
    fn manual_focus_change_stops_auto_paste_but_keeps_text_available_for_copy() {
        let mut state = ProbeSession::default();
        state.begin(1);
        state.stop_auto_paste();
        state.input(1, 9, false);
        state.ime_end(1);
        state.release(1);
        assert!(!state.auto_paste);
        assert!(state.claim_delivery(1, state.revision));
        assert_eq!(state.chars, 9);
    }

    #[test]
    fn claimed_or_cancelled_output_cannot_reopen_from_late_events() {
        let mut state = ProbeSession::default();
        state.begin(1);
        state.input(1, 9, false);
        state.release(1);
        state.ime_end(1);
        assert!(state.claim_delivery(1, state.revision));
        assert!(!state.ime_end(1));
        assert!(!state.input(1, 12, true));
        assert!(!state.claim_delivery(1, state.revision));
        state.cancel();
        assert!(!state.ime_end(1));
        assert!(!state.claim_delivery(1, 0));
    }

    struct Host {
        calls: Vec<&'static str>,
        busy: bool,
        copy_error: bool,
        allow: Result<(), DeliveryFailure>,
        restore: bool,
        verified: Result<(), DeliveryFailure>,
        paste: bool,
    }
    impl Default for Host {
        fn default() -> Self {
            Self {
                calls: vec![],
                busy: false,
                copy_error: false,
                allow: Ok(()),
                restore: true,
                verified: Ok(()),
                paste: true,
            }
        }
    }
    impl DeliveryHost for Host {
        fn copy_verified(&mut self, _: &str) -> Result<Option<()>, DeliveryFailure> {
            self.calls.push("copy_verified");
            if self.copy_error {
                Err(DeliveryFailure::CopyFailed)
            } else if self.busy {
                Ok(None)
            } else {
                Ok(Some(()))
            }
        }
        fn allow_restore(&mut self) -> Result<(), DeliveryFailure> {
            self.calls.push("allow_restore");
            self.allow
        }
        fn hide_receiver(&mut self) -> bool {
            self.calls.push("hide");
            true
        }
        fn restore_target(&mut self) -> bool {
            self.calls.push("restore");
            self.restore
        }
        fn verify_before_paste(&mut self) -> Result<(), DeliveryFailure> {
            self.calls.push("verify_before_paste");
            self.verified
        }
        fn paste_once(&mut self) -> bool {
            self.calls.push("ctrl_v");
            self.paste
        }
        fn retain_receiver(&mut self) {
            self.calls.push("retain");
        }
    }
    #[test]
    fn whole_text_is_copied_before_hide_restore_and_one_ctrl_v() {
        let mut host = Host::default();
        assert_eq!(
            deliver_with(&mut host, "第一行\n第二行🙂"),
            DeliveryOutcome::Submitted
        );
        assert_eq!(
            host.calls,
            vec![
                "copy_verified",
                "allow_restore",
                "hide",
                "restore",
                "verify_before_paste",
                "ctrl_v"
            ]
        );
    }
    #[test]
    fn clipboard_lock_does_not_hide_or_change_focus_and_can_be_retried() {
        let mut host = Host {
            busy: true,
            ..Default::default()
        };
        assert_eq!(
            deliver_with(&mut host, "text"),
            DeliveryOutcome::ClipboardBusy
        );
        assert_eq!(host.calls, vec!["copy_verified"]);
        host.busy = false;
        assert_eq!(deliver_with(&mut host, "text"), DeliveryOutcome::Submitted);
        assert_eq!(
            host.calls.iter().filter(|&&call| call == "ctrl_v").count(),
            1
        );
    }
    #[test]
    fn changed_target_or_manual_focus_change_never_hides_or_pastes() {
        for reason in [
            DeliveryFailure::TargetChanged,
            DeliveryFailure::SourceChanged,
            DeliveryFailure::Interrupted,
        ] {
            let mut host = Host {
                allow: Err(reason),
                ..Default::default()
            };
            assert_eq!(
                deliver_with(&mut host, "text"),
                DeliveryOutcome::Kept(reason)
            );
            assert_eq!(host.calls, vec!["copy_verified", "allow_restore"]);
        }
    }
    #[test]
    fn failed_focus_restore_keeps_the_text_and_never_sends_keys() {
        let mut host = Host {
            restore: false,
            ..Default::default()
        };
        assert_eq!(
            deliver_with(&mut host, "text"),
            DeliveryOutcome::Kept(DeliveryFailure::RestoreFailed)
        );
        assert_eq!(host.calls.last(), Some(&"retain"));
        assert!(!host.calls.contains(&"ctrl_v"));
    }
    #[test]
    fn changed_clipboard_and_partial_or_rejected_paste_are_not_replayed() {
        let mut host = Host {
            verified: Err(DeliveryFailure::ClipboardChanged),
            ..Default::default()
        };
        assert_eq!(
            deliver_with(&mut host, "text"),
            DeliveryOutcome::Kept(DeliveryFailure::ClipboardChanged)
        );
        assert!(!host.calls.contains(&"ctrl_v"));
        let mut rejected = Host {
            paste: false,
            ..Default::default()
        };
        assert_eq!(
            deliver_with(&mut rejected, "text"),
            DeliveryOutcome::Kept(DeliveryFailure::PasteRejected)
        );
        assert_eq!(
            rejected
                .calls
                .iter()
                .filter(|&&call| call == "ctrl_v")
                .count(),
            1
        );
    }
}
