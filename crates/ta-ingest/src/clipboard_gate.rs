/// Pure policy gate for clipboard change events. It never reads or stores clipboard text.
#[derive(Debug, Clone)]
pub struct ClipboardGate {
    enabled: bool,
    paused: bool,
    last_sequence: Option<u32>,
    recent_sequences: [Option<u32>; 64],
    next_slot: usize,
}

impl ClipboardGate {
    /// Start accepting new clipboard change events.
    pub fn new() -> Self {
        Self {
            enabled: true,
            paused: false,
            last_sequence: None,
            recent_sequences: [None; 64],
            next_slot: 0,
        }
    }

    /// Re-enable capture, using the current clipboard sequence as a stale-event cutoff.
    pub fn enable_at(&mut self, current_sequence: u32) {
        self.enabled = true;
        self.last_sequence = Some(current_sequence);
    }

    /// Disable capture without reading the clipboard.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Pause capture without reading the clipboard.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume capture, ignoring events at or before the current clipboard sequence.
    pub fn resume_at(&mut self, current_sequence: u32) {
        self.paused = false;
        self.last_sequence = Some(current_sequence);
    }

    /// Evaluate a vetted clipboard event. A self-copy flag suppresses only this event.
    pub fn accept<'a>(
        &mut self,
        sequence: u32,
        text: &'a str,
        is_self_copy: bool,
    ) -> Option<&'a str> {
        if !self.enabled || self.paused {
            return None;
        }
        if let Some(last) = self.last_sequence {
            let change = sequence.wrapping_sub(last);
            if change == 0 || change >= (1_u32 << 31) {
                return None;
            }
        }
        if self.recent_sequences.contains(&Some(sequence)) {
            return None;
        }
        self.last_sequence = Some(sequence);
        self.recent_sequences[self.next_slot] = Some(sequence);
        self.next_slot = (self.next_slot + 1) % self.recent_sequences.len();

        if is_self_copy || text.trim().is_empty() {
            return None;
        }
        Some(text)
    }
}

impl Default for ClipboardGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ClipboardGate;

    #[test]
    fn pause_resume_requires_a_new_change() {
        let mut gate = ClipboardGate::new();
        assert_eq!(gate.accept(1, "old", false), Some("old"));
        gate.pause();
        assert_eq!(gate.accept(2, "paused", false), None);
        gate.resume_at(2);
        assert_eq!(gate.accept(2, "old", false), None);
        assert_eq!(gate.accept(3, "new", false), Some("new"));
        assert_eq!(gate.accept(1, "queued old event", false), None);
    }

    #[test]
    fn suppresses_own_copy_without_poisoning_identical_external_text() {
        let mut gate = ClipboardGate::new();
        assert_eq!(gate.accept(1, "same", true), None);
        assert_eq!(gate.accept(2, "same", false), Some("same"));
    }

    #[test]
    fn resume_baseline_accepts_wrapped_next_change_only() {
        let mut gate = ClipboardGate::new();
        gate.pause();
        gate.resume_at(u32::MAX);
        assert_eq!(gate.accept(u32::MAX - 1, "old", false), None);
        assert_eq!(gate.accept(0, "wrapped", false), Some("wrapped"));
        assert_eq!(gate.accept(0, "repeat", false), None);
    }

    #[test]
    fn whitespace_disable_duplicates_and_bursts_are_handled() {
        let mut gate = ClipboardGate::new();
        assert_eq!(gate.accept(1, " \n\t", false), None);
        assert_eq!(gate.accept(2, "text", false), Some("text"));
        assert_eq!(gate.accept(2, "text", false), None);
        for seq in 3..53 {
            assert_eq!(gate.accept(seq, "text", false), Some("text"));
        }
        gate.disable();
        assert_eq!(gate.accept(53, "later", false), None);
        gate.enable_at(53);
        assert_eq!(gate.accept(53, "stale", false), None);
        assert_eq!(gate.accept(54, "fresh", false), Some("fresh"));
    }
}
