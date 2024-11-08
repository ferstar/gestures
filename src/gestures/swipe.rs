use knuffel::{Decode, DecodeScalar};

#[derive(Decode, Debug, Clone, PartialEq, Eq)]
pub struct Swipe {
    #[knuffel(property)]
    pub direction: SwipeDir,
    #[knuffel(property)]
    pub fingers: i32,
    #[knuffel(property)]
    pub update: Option<String>,
    #[knuffel(property)]
    pub start: Option<String>,
    #[knuffel(property)]
    pub end: Option<String>,
    #[knuffel(property)]
    pub acceleration: Option<i8>,
    #[knuffel(property)]
    pub mouse_up_delay: Option<i64>,
}

/// Direction of swipe gestures
///
/// NW  N  NE
/// W   C   E
/// SW  S  SE
#[derive(DecodeScalar, Debug, Clone, PartialEq, Eq)]
pub enum SwipeDir {
    Any,
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl SwipeDir {
    pub fn dir(x: f64, y: f64) -> SwipeDir {
        if x == 0.0 && y == 0.0 {
            return SwipeDir::Any;
        }

        let angle = y.atan2(x); // Range: -π to π

        match angle {
            a if a < -2.7489 => SwipeDir::W,  // -π to -7π/8
            a if a < -1.9635 => SwipeDir::NW, // -7π/8 to -5π/8
            a if a < -1.1781 => SwipeDir::N,  // -5π/8 to -3π/8
            a if a < -0.3927 => SwipeDir::NE, // -3π/8 to -π/8
            a if a < 0.3927 => SwipeDir::E,   // -π/8 to π/8
            a if a < 1.1781 => SwipeDir::SE,  // π/8 to 3π/8
            a if a < 1.9635 => SwipeDir::S,   // 3π/8 to 5π/8
            a if a < 2.7489 => SwipeDir::SW,  // 5π/8 to 7π/8
            _ => SwipeDir::W,                 // 7π/8 to π
        }
    }
}
