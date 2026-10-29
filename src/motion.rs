// Analytic critically damped spring: stable after a delayed frame, with no
// overshoot. Once settled, the UI can stop its timer entirely.
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub value: f32,
    pub target: f32,
    velocity: f32,
}
impl Spring {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            velocity: 0.0,
        }
    }
    pub fn set(&mut self, target: f32) {
        self.target = target;
    }
    pub fn snap(&mut self, value: f32) {
        *self = Self::new(value);
    }
    pub fn active(&self) -> bool {
        (self.value - self.target).abs() > 0.002 || self.velocity.abs() > 0.02
    }
    pub fn step(&mut self, seconds: f32) {
        let omega = 30.0;
        let offset = self.value - self.target;
        let c = self.velocity + omega * offset;
        let decay = (-omega * seconds).exp();
        self.value = self.target + (offset + c * seconds) * decay;
        self.velocity = (self.velocity - omega * c * seconds) * decay;
        if !self.active() {
            self.snap(self.target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spring_is_continuous_on_reversal_and_eventually_stops() {
        let mut s = Spring::new(0.0);
        s.set(1.0);
        for _ in 0..5 {
            s.step(0.016);
        }
        assert!(s.value > 0.0 && s.value < 1.0);
        let current = s.value;
        s.set(0.0);
        assert_eq!(s.value, current);
        for _ in 0..60 {
            s.step(0.016);
        }
        assert_eq!(s.value, 0.0);
        assert!(!s.active());
        s.set(100.0);
        s.step(5.0);
        assert_eq!(s.value, 100.0);
        assert!(!s.active());
    }
}
