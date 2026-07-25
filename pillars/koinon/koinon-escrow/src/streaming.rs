use koinon_ledger::AccountId;

pub type StreamId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamState {
    Active,
    Paused,
    Stopped,
    Completed,
}

#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub rate_per_second: u64,
    pub duration_seconds: u64,
    pub token: StreamToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamToken {
    Oikos,
    Koin,
}

#[derive(Debug, Clone)]
pub struct StreamingPayment {
    pub id: StreamId,
    pub sender: AccountId,
    pub receiver: AccountId,
    pub total_amount: u128,
    pub streamed_amount: u128,
    pub config: StreamConfig,
    pub state: StreamState,
    pub start_time: u64,
    pub pause_time: Option<u64>,
}

impl StreamingPayment {
    pub fn new(
        id: StreamId,
        sender: AccountId,
        receiver: AccountId,
        total_amount: u128,
        config: StreamConfig,
        current_time: u64,
    ) -> Self {
        Self {
            id,
            sender,
            receiver,
            total_amount,
            streamed_amount: 0,
            config,
            state: StreamState::Active,
            start_time: current_time,
            pause_time: None,
        }
    }

    pub fn streamed_so_far(&self, current_time: u64) -> u128 {
        match self.state {
            StreamState::Active => {
                let elapsed = current_time.saturating_sub(self.start_time);
                let streamed = elapsed as u128 * self.config.rate_per_second as u128;
                streamed.min(self.total_amount)
            }
            StreamState::Paused => {
                self.pause_time
                    .map(|pt| {
                        let elapsed = pt.saturating_sub(self.start_time);
                        let streamed = elapsed as u128 * self.config.rate_per_second as u128;
                        streamed.min(self.total_amount)
                    })
                    .unwrap_or(self.streamed_amount)
            }
            _ => self.streamed_amount,
        }
    }

    pub fn update_streamed(&mut self, current_time: u64) {
        if self.state == StreamState::Active {
            self.streamed_amount = self.streamed_so_far(current_time);
        }
    }

    pub fn can_complete(&self, current_time: u64) -> bool {
        self.state == StreamState::Active && self.streamed_so_far(current_time) >= self.total_amount
    }

    pub fn try_complete(&mut self, current_time: u64) -> bool {
        if self.can_complete(current_time) {
            self.streamed_amount = self.total_amount;
            self.state = StreamState::Completed;
            true
        } else {
            false
        }
    }

    pub fn remaining(&self, current_time: u64) -> u128 {
        self.total_amount
            .saturating_sub(self.streamed_so_far(current_time))
    }

    pub fn pause(&mut self, current_time: u64) {
        if self.state == StreamState::Active {
            self.streamed_amount = self.streamed_so_far(current_time);
            self.state = StreamState::Paused;
            self.pause_time = Some(current_time);
        }
    }

    pub fn resume(&mut self, current_time: u64) {
        if self.state == StreamState::Paused {
            let pre_streamed = self.streamed_amount / self.config.rate_per_second as u128;
            self.start_time = current_time.saturating_sub(pre_streamed as u64);
            self.pause_time = None;
            self.state = StreamState::Active;
        }
    }

    pub fn can_stop(&self) -> bool {
        matches!(self.state, StreamState::Active | StreamState::Paused)
    }

    pub fn stop(&mut self) {
        if self.can_stop() {
            self.state = StreamState::Stopped;
        }
    }

    pub fn conservation_check(&self) -> bool {
        self.streamed_amount <= self.total_amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> StreamConfig {
        StreamConfig {
            rate_per_second: 10,
            duration_seconds: 100,
            token: StreamToken::Koin,
        }
    }

    fn make_stream(total: u128, start_time: u64) -> StreamingPayment {
        StreamingPayment::new(1, 100, 200, total, test_config(), start_time)
    }

    #[test]
    fn new_stream_is_active() {
        let s = make_stream(1000, 0);
        assert_eq!(s.state, StreamState::Active);
        assert_eq!(s.streamed_amount, 0);
    }

    #[test]
    fn streamed_so_far_active() {
        let s = make_stream(1000, 100);
        assert_eq!(s.streamed_so_far(110), 100); // 10 seconds * 10 rate
    }

    #[test]
    fn streamed_so_far_capped_at_total() {
        let s = make_stream(50, 100);
        assert_eq!(s.streamed_so_far(200), 50); // would be 1000 but capped at 50
    }

    #[test]
    fn update_streamed_captures_amount() {
        let mut s = make_stream(1000, 100);
        s.update_streamed(110);
        assert_eq!(s.streamed_amount, 100);
    }

    #[test]
    fn update_streamed_noop_when_paused() {
        let mut s = make_stream(1000, 100);
        s.pause(105);
        s.update_streamed(200);
        assert_eq!(s.streamed_amount, 50); // paused at 5s * 10 rate
    }

    #[test]
    fn can_complete_when_fully_streamed() {
        let s = make_stream(100, 0);
        assert!(s.can_complete(10)); // 10 * 10 = 100
    }

    #[test]
    fn cannot_complete_before_full() {
        let s = make_stream(100, 0);
        assert!(!s.can_complete(9));
    }

    #[test]
    fn try_complete_transitions_to_completed() {
        let mut s = make_stream(100, 0);
        assert!(s.try_complete(10));
        assert_eq!(s.state, StreamState::Completed);
        assert_eq!(s.streamed_amount, 100);
    }

    #[test]
    fn try_complete_fails_when_not_ready() {
        let mut s = make_stream(100, 0);
        assert!(!s.try_complete(5));
        assert_eq!(s.state, StreamState::Active);
    }

    #[test]
    fn try_complete_fails_when_paused() {
        let mut s = make_stream(100, 0);
        s.pause(5);
        assert!(!s.try_complete(100));
    }

    #[test]
    fn pause_captures_amount() {
        let mut s = make_stream(1000, 100);
        s.pause(110);
        assert_eq!(s.state, StreamState::Paused);
        assert_eq!(s.streamed_amount, 100);
        assert_eq!(s.pause_time, Some(110));
    }

    #[test]
    fn pause_noop_when_not_active() {
        let mut s = make_stream(1000, 100);
        s.pause(110);
        s.pause(120);
        assert_eq!(s.streamed_amount, 100); // unchanged
    }

    #[test]
    fn resume_sets_effective_start_time() {
        let mut s = make_stream(1000, 0);
        s.pause(10); // streamed 100
        s.resume(50); // effective start = 50 - 10 = 40
        assert_eq!(s.state, StreamState::Active);
        assert_eq!(s.start_time, 40);
        assert_eq!(s.pause_time, None);
    }

    #[test]
    fn resume_noop_when_not_paused() {
        let mut s = make_stream(1000, 0);
        s.resume(50);
        assert_eq!(s.state, StreamState::Active);
        assert_eq!(s.start_time, 0);
    }

    #[test]
    fn resume_after_pause_streams_correctly() {
        let mut s = make_stream(1000, 0);
        s.pause(10); // streamed 100
        s.resume(50); // effective start = 40
        assert_eq!(s.streamed_so_far(60), 200); // (60-40)*10 = 200 (100 pre-pause + 100 post)
    }

    #[test]
    fn remaining_decreases() {
        let s = make_stream(1000, 0);
        assert_eq!(s.remaining(5), 950);
        assert_eq!(s.remaining(10), 900);
    }

    #[test]
    fn can_stop_active() {
        let s = make_stream(1000, 0);
        assert!(s.can_stop());
    }

    #[test]
    fn can_stop_paused() {
        let mut s = make_stream(1000, 0);
        s.pause(10);
        assert!(s.can_stop());
    }

    #[test]
    fn cannot_stop_completed() {
        let mut s = make_stream(100, 0);
        s.try_complete(10);
        assert!(!s.can_stop());
    }

    #[test]
    fn stop_sets_stopped() {
        let mut s = make_stream(1000, 0);
        s.stop();
        assert_eq!(s.state, StreamState::Stopped);
    }

    #[test]
    fn conservation_check_passes() {
        let mut s = make_stream(1000, 0);
        s.update_streamed(50);
        assert!(s.conservation_check());
    }

    #[test]
    fn conservation_check_never_breaks_underflow() {
        let s = make_stream(100, 0);
        assert!(s.conservation_check());
    }

    #[test]
    fn full_lifecycle() {
        let mut s = make_stream(200, 0);
        assert_eq!(s.streamed_so_far(5), 50);
        s.pause(10);
        assert_eq!(s.streamed_amount, 100);
        assert_eq!(s.state, StreamState::Paused);
        s.resume(20);
        assert_eq!(s.state, StreamState::Active);
        // effective_start = 20 - (100/10) = 10, so at t=30: (30-10)*10 = 200
        assert!(s.can_complete(30));
        assert!(s.try_complete(30));
        assert_eq!(s.state, StreamState::Completed);
        assert_eq!(s.streamed_amount, 200);
        assert!(s.conservation_check());
    }
}
